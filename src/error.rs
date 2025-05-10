use std::error::Error;
use crate::DbRequest;

#[derive(Debug)]
pub enum TrackerError {
    DbManagerExited,
    ServerError,
    MempoolIndexerError,
    Shutdown,
    ParsingError,
    SendError,
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

impl Error for TrackerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl std::fmt::Display for TrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<tokio::sync::mpsc::error::SendError<DbRequest>>  for TrackerError {
    fn from(_: tokio::sync::mpsc::error::SendError<DbRequest>) -> Self {
        Self::SendError
    }
}
