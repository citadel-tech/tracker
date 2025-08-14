use crate::db::model::MempoolTx;
use crate::server::tracker_monitor::monitor_systems;
use crate::status;
use crate::types::DbRequest;
use crate::types::TrackerClientToServer;
use crate::types::TrackerServerToClient;
use crate::utils::read_message;
use crate::utils::send_message;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tracing::error;
use tracing::info;

pub async fn run(
    db_tx: Sender<DbRequest>,
    status_tx: status::Sender,
    address: String,
    #[cfg(not(feature = "integration-test"))] socks_port: u16,
    onion_address: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port = address
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .unwrap_or(8080);
    let server = TcpListener::bind(&address).await?;

    tokio::spawn(monitor_systems(
        db_tx.clone(),
        status_tx.clone(),
        #[cfg(not(feature = "integration-test"))]
        socks_port,
        onion_address,
        port,
    ));

    info!("Tracker server listening on {}", address);

    while let Ok((stream, client_addr)) = server.accept().await {
        info!("Accepted connection from {}", client_addr);
        let db_tx_clone = db_tx.clone();
        tokio::spawn(async move { handle_client(stream, db_tx_clone).await });
    }

    Ok(())
}
async fn handle_client(mut stream: TcpStream, db_tx: Sender<DbRequest>) {
    let (read_half, write_half) = stream.split();
    let mut reader = BufReader::new(read_half);
    let mut writer = BufWriter::new(write_half);

    loop {
        let buffer = match read_message(&mut reader).await {
            Ok(buf) => buf,
            Err(e) if e.io_error_kind() == Some(std::io::ErrorKind::UnexpectedEof) => {
                info!("Client disconnected.");
                break;
            }
            Err(e) => {
                error!("Failed to read message: {}", e);
                break;
            }
        };

        let request: TrackerClientToServer = match serde_cbor::de::from_reader(&buffer[..]) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to deserialize client request: {e}");
                break;
            }
        };

        match request {
            TrackerClientToServer::Get => {
                info!("Received Get request taker");
                let (resp_tx, mut resp_rx) = mpsc::channel(1);
                let db_request = DbRequest::QueryActive(resp_tx);

                if let Err(e) = db_tx.send(db_request).await {
                    error!("Failed to send DB request: {e}");
                    break;
                }

                let response = resp_rx.recv().await;
                info!("Response: {:?}", response);

                if let Some(addresses) = response {
                    let message = TrackerServerToClient::Address { addresses };
                    if let Err(e) = send_message(&mut writer, &message).await {
                        error!("Failed to send response to client: {e}");
                        break;
                    }
                }
            }

            TrackerClientToServer::Post { metadata: _ } => {
                todo!()
            }

            TrackerClientToServer::Pong { address: _ } => {
                todo!()
            }
            TrackerClientToServer::Watch { outpoint } => {
                info!("Received a watch request from client: {outpoint:?}");

                let (resp_tx, mut resp_rx) = mpsc::channel::<Vec<MempoolTx>>(1);

                let db_request = DbRequest::WatchUtxo(outpoint, resp_tx);

                if let Err(e) = db_tx.send(db_request).await {
                    error!("Failed to send DB request: {e}");
                    break;
                }

                let response = resp_rx.recv().await;
                info!("Response: {:?}", response);

                if let Some(mempool_tx) = response {
                    let message = TrackerServerToClient::WatchResponse { mempool_tx };
                    if let Err(e) = send_message(&mut writer, &message).await {
                        error!("Failed to send response to client: {e}");
                        break;
                    }
                }
            }
        }
    }

    info!("Connection handler exiting.");
}
