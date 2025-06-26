use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;
use tracing::info;

use crate::{
    error::TrackerError,
    status::{self, Status},
    types::{DbRequest, ServerInfo},
};

pub async fn run(mut rx: Receiver<DbRequest>, status_tx: status::Sender) {
    let mut servers: HashMap<String, ServerInfo> = HashMap::new();
    info!("DB manager started");
    while let Some(request) = rx.recv().await {
        match request {
            DbRequest::Add(addr, info) => {
                info!("Add request intercepted: address: {addr:?}, info: {info:?}");
                servers.insert(addr, info);
            }
            DbRequest::Query(addr, resp_tx) => {
                info!("Query request intecepted");
                let result = servers.get(&addr).cloned();
                let _ = resp_tx.send(result).await;
            }
            DbRequest::Update(addr, server_info) => {
                info!("Update request intercepted");
                servers.insert(addr, server_info);
            }
            DbRequest::QueryAll(resp_tx) => {
                info!("Query all request intercepted");
                let response: Vec<(String, ServerInfo)> =
                    servers.iter().map(|e| (e.0.clone(), e.1.clone())).collect();
                let _ = resp_tx.send(response).await;
            }
            DbRequest::QueryActive(resp_tx) => {
                info!("Query active intercepted");
                let response: Vec<String> = servers
                    .iter()
                    .filter(|x| !x.1.stale)
                    .map(|e| e.0.clone())
                    .collect();
                let _ = resp_tx.send(response).await;
            }
        }
    }

    let _ = status_tx
        .send(Status {
            state: status::State::DBShutdown(TrackerError::DbManagerExited),
        })
        .await;
}
