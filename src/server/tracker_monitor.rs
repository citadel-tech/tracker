use std::error::Error;
use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    sync::mpsc::Sender,
    time::{sleep, Instant},
};
use tracing::{ info, error };

use crate::{handle_result, status, types::{DbRequest, ServerInfo}};

const COOLDOWN_PERIOD: u64 = 5 * 60;

pub async fn monitor_systems(
    db_tx: Sender<DbRequest>,
    status_tx: status::Sender,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Starting to monitor other maker services");
    loop {

        sleep(Duration::from_secs(1000)).await;

        let (response_tx, mut response_rx) = tokio::sync::mpsc::channel(1);
        if db_tx.send(DbRequest::QueryAll(response_tx)).await.is_err() {
            continue;
        }

        if let Some(response) = response_rx.recv().await {
            for (address, server_info) in response {
                let cooldown_duration = Duration::from_secs(COOLDOWN_PERIOD);
                if server_info.cooldown.elapsed() <= cooldown_duration {
                    continue;
                }

                match TcpStream::connect(&address).await {
                    Ok(stream) => {
                        let (reader, writer) = stream.into_split();
                        let mut buf_reader = BufReader::new(reader);
                        let mut buf_writer = BufWriter::new(writer);

                        if let Err(e) = buf_writer.write_all(b"send").await {
                            error!("Error: {:?}", e);
                            continue;
                        }

                        let mut response = String::new();
                        let n = handle_result!(status_tx, buf_reader.read_to_string(&mut response).await);
                        info!("Number of bytes read: {:?}", n);

                        let updated_info = ServerInfo {
                            onion_address: response,
                            cooldown: Instant::now(),
                            stale: false,
                        };

                        let _ = db_tx.send(DbRequest::Update(address, updated_info)).await;
                    }
                    Err(_) => {
                        if !server_info.stale {
                            let updated_info = ServerInfo {
                                stale: true,
                                ..server_info
                            };
                            let _ = db_tx.send(DbRequest::Update(address, updated_info)).await;
                        }
                    }
                }
            }
        }
    }
}
