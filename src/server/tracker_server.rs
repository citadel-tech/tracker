use crate::handle_result;
use crate::server::tracker_monitor::monitor_systems;
use crate::status;
use crate::types::DbRequest;
use crate::types::DnsRequest;
use crate::types::DnsResponse;
use crate::utils::read_message;
use crate::utils::send_message;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tracing::info;

pub async fn run(
    db_tx: Sender<DbRequest>,
    status_tx: status::Sender,
    address: String,
    socks_port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = TcpListener::bind(&address).await?;

    tokio::spawn(monitor_systems(
        db_tx.clone(),
        status_tx.clone(),
        socks_port,
    ));

    info!("Tracker server listening on {}", address);

    while let Ok((stream, client_addr)) = server.accept().await {
        info!("Accepted connection from {}", client_addr);
        let status_tx_clone = status_tx.clone();
        let db_tx_clone = db_tx.clone();
        tokio::spawn(async move { handle_client(stream, status_tx_clone, db_tx_clone).await });
    }

    Ok(())
}

async fn handle_client(mut stream: TcpStream, status_tx: status::Sender, db_tx: Sender<DbRequest>) {
    loop {
        let buffer = handle_result!(status_tx, read_message(&mut stream).await);
        let request: DnsRequest =
            handle_result!(status_tx, serde_cbor::de::from_reader(&buffer[..]));

        match request {
            DnsRequest::Get => {
                info!("Received Get request taker");
                let (resp_tx, mut resp_rx) = mpsc::channel(1);
                let db_request = DbRequest::QueryActive(resp_tx);
                handle_result!(status_tx, db_tx.send(db_request).await);
                let response = resp_rx.recv().await;
                if let Some(addresses) = response {
                    let message = DnsResponse::Address { addresses };
                    _ = send_message(&mut stream, &message).await;
                }
            }
            DnsRequest::Post { metadata: _ } => {
                todo!()
            }
            DnsRequest::Pong { address: _ } => {
                todo!()
            }
        }
    }
}
