use tokio::{sync::mpsc::Sender, time::Instant};

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub onion_address: String,
    pub cooldown: Instant,
    pub stale: bool
}

pub enum DbRequest {
    Add(String, ServerInfo),
    Query(String, Sender<Option<ServerInfo>>),
    Update(String, ServerInfo),
    QueryAll(Sender<Vec<(String, ServerInfo)>>),
    QueryActive(Sender<Vec<(String, ServerInfo)>>)
}
