use crate::DbRequest;
use std::error::Error;

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
    SerdeCbor(serde_cbor::Error),
    General(String),
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
        write!(f, "{self:?}")
    }
}

impl From<tokio::sync::mpsc::error::SendError<DbRequest>> for TrackerError {
    fn from(_: tokio::sync::mpsc::error::SendError<DbRequest>) -> Self {
        Self::SendError
    }
}

impl From<serde_cbor::Error> for TrackerError {
    fn from(value: serde_cbor::Error) -> Self {
        Self::SerdeCbor(value)
    }
}

impl TrackerError {
    pub fn io_error_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            TrackerError::IOError(e) => Some(e.kind()),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            TrackerError::DbManagerExited => "DbManagerExited",
            TrackerError::ServerError => "ServerError",
            TrackerError::MempoolIndexerError => "MempoolIndexerError",
            TrackerError::Shutdown => "Shutdown",
            TrackerError::ParsingError => "ParsingError",
            TrackerError::SendError => "SendError",
            TrackerError::IOError(_) => "IOError",
            TrackerError::RPCError(_) => "RPCError",
            TrackerError::SerdeCbor(_) => "SerdeCbor",
            TrackerError::General(_) => "General",
        }
    }
}
