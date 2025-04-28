#[derive(Debug)]
pub enum TrackerError {
    DbManagerExited,
    ServerError,
    MempoolIndexerError,
    Shutdown,
    IOError(std::io::Error),
    RPCError(bitcoincore_rpc::Error),
}

impl From<std::io::Error> for TrackerError {
    fn from(value: std::io::Error) -> Self {
        TrackerError::IOError(value)
    }
}

impl From<bitcoincore_rpc::Error> for TrackerError {
    fn from(value: bitcoincore_rpc::Error) -> Self {
        TrackerError::RPCError(value)
    }
}
