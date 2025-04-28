use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub onion_address: String,
    pub rate: f64,
    pub uptime: u64,
}

pub enum DbRequest {
    Add(String, ServerInfo),
    Query(String, Sender<Option<ServerInfo>>),
    Update(String, Box<dyn FnOnce(&mut ServerInfo) + Send>),
}
