use std::time::Duration;

use tokio::{
    io::BufWriter,
    sync::mpsc::Sender,
    time::{Instant, sleep},
};
use tokio_socks::tcp::Socks5Stream;
use tracing::{info, warn};

use crate::{
    error::TrackerError,
    handle_result, status,
    types::{DbRequest, DnsRequest, DnsResponse, ServerInfo},
    utils::{read_message, send_message},
};

use tokio::io::BufReader;

const COOLDOWN_PERIOD: u64 = 5 * 60;
pub async fn monitor_systems(
    db_tx: Sender<DbRequest>,
    status_tx: status::Sender,
    socks_port: u16,
) -> Result<(), TrackerError> {
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

                let mut success = false;
                for attempt in 1..=3 {
                    let connect_result = Socks5Stream::connect(
                        format!("127.0.0.1:{socks_port:?}").as_str(),
                        address.clone(),
                    )
                    .await;

                    match connect_result {
                        Ok(mut stream) => {
                            success = true;

                            let (read_half, write_half) = stream.split();

                            let mut reader = BufReader::new(read_half);

                            let mut writer = BufWriter::new(write_half);

                            let message = DnsResponse::Ping;
                            _ = send_message(&mut writer, &message).await;

                            let buffer = handle_result!(status_tx, read_message(&mut reader).await);
                            let response: DnsRequest =
                                handle_result!(status_tx, serde_cbor::de::from_reader(&buffer[..]));

                            if let DnsRequest::Pong { address } = response {
                                let updated_info = ServerInfo {
                                    onion_address: address.clone(),
                                    cooldown: Instant::now(),
                                    stale: false,
                                };
                                let _ = db_tx.send(DbRequest::Update(address, updated_info)).await;
                            }

                            break;
                        }

                        Err(e) => {
                            warn!(
                                "Failed to connect to {} (attempt {}/3): {}",
                                address, attempt, e
                            );
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }

                if !success && !server_info.stale {
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
