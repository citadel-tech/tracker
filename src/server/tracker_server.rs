use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tracing::{info, error};
use tokio::io::BufReader;
use tokio::io::BufWriter;
use crate::error::TrackerError;
use crate::server::tracker_monitor::monitor_systems;
use tokio::io::AsyncBufReadExt;
use crate::status;
use crate::types::DbRequest;

pub async fn run(db_tx: Sender<DbRequest>, status_tx: status::Sender, address: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{

    let server = TcpListener::bind(&address).await?;

    tokio::spawn(monitor_systems(db_tx.clone(), status_tx.clone()));

    info!("Tracker server listening on {}", address);

    while let Ok((stream, client_addr)) = server.accept().await {
        info!("Accepted connection from {}", client_addr);
        let status_tx_clone = status_tx.clone();
        let db_tx_clone = db_tx.clone();
        tokio::spawn(async move {
            let (reader_part, writer_part) = stream.into_split();
            let mut reader = BufReader::new(reader_part);
            let mut writer = BufWriter::new(writer_part);
            let mut line = String::new();
            loop {
                line.clear();
                let bytes_read = match reader.read_line(&mut line).await {
                     Ok(n) => n,
                     Err(e) => {
                        error!("Error reading from {}: {}", client_addr, e);
                        let _ = status_tx_clone.send(status::Status {
                            state: status::State::ServerShutdown(TrackerError::IOError(e)) 
                        }).await;
                        return;
                     }
                };

                info!("Bytes read: {:?}", bytes_read);

                let request_message = line.trim();

                if request_message.is_empty() {
                    continue;
                }

                info!("Received message from {}: {:?}", client_addr, request_message);

                let response_message: String;

                if request_message.starts_with("QUERY ") {

                    let (resp_tx, mut resp_rx) = mpsc::channel(1); 
                    let db_request = DbRequest::QueryActive(resp_tx);
                    let send_res = db_tx_clone.send(db_request).await;

                    if let Err(e) = send_res {
                        error!("Error sending DB query for {}: {}", client_addr, e);
                         let _ = status_tx_clone.send(status::Status {
                             state: status::State::ServerShutdown(TrackerError::DbManagerExited) // Example error
                         }).await;
                         response_message = "ERROR: DB query failed".to_string();
                    } else {
                        match resp_rx.recv().await {
                            Some(server_info) => {
                                response_message = format!("OK: {:?}", server_info);
                            }
                            None => {
                                response_message = "NOT_FOUND".to_string();
                            }
                        }
                    }
                } else if request_message.starts_with("ADD ") {
                     response_message = "ERROR: Add command not implemented via server".to_string();
                }
                 else {
                    response_message = format!("ERROR: Unknown command '{}'", request_message); // Custom protocol error
                }

                let write_res = writer.write_all(response_message.as_bytes()).await;
                if let Err(e) = write_res {
                     error!("Error writing to {}: {}", client_addr, e);
                    let _ = status_tx_clone.send(status::Status {
                        state: status::State::ServerShutdown(TrackerError::IOError(e))
                    }).await;
                    return; 
                 }
                 if let Err(e) = writer.write_all(b"\n").await {
                      error!("Error writing newline to {}: {}", client_addr, e);
                      let _ = status_tx_clone.send(status::Status {
                        state: status::State::ServerShutdown(TrackerError::IOError(e))
                    }).await;
                 }
                if let Err(e) = writer.flush().await {
                    error!("Error flushing writer for {}: {}", client_addr, e);
                     let _ = status_tx_clone.send(status::Status {
                        state: status::State::ServerShutdown(TrackerError::IOError(e))
                    }).await;
                    return;
                }
                 println!("Sent response to {}", client_addr);
            }
        });
    }

    Ok(())

}
