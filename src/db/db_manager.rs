use std::{collections::HashMap, sync::mpsc::Receiver};

use crate::{
    error::TrackerError,
    status::{self, Status},
    types::{DbRequest, ServerInfo},
};

pub fn run(rx: Receiver<DbRequest>, status_tx: status::Sender) {
    let mut servers: HashMap<String, ServerInfo> = HashMap::new();
    while let Ok(request) = rx.recv() {
        match request {
            DbRequest::Add(addr, info) => {
                servers.insert(addr, info);
            }
            DbRequest::Query(addr, resp_tx) => {
                let result = servers.get(&addr).cloned();
                let _ = resp_tx.send(result);
            }
            DbRequest::Update(addr, update_fn) => {
                if let Some(server) = servers.get_mut(&addr) {
                    update_fn(server)
                }
            }
        }
    }

    let _ = status_tx.send(Status {
        state: status::State::DBShutdown(TrackerError::DbManagerExited),
    });
}
