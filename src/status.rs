use crate::{error::TrackerError, handle_error::ErrorBranch};
use std::sync::mpsc::{self, SendError};

#[derive(Debug)]
pub enum Sender {
    Mempool(mpsc::Sender<Status>),
    Server(mpsc::Sender<Status>),
    DBManager(mpsc::Sender<Status>),
}

impl Sender {
    pub fn send(&self, status: Status) -> Result<(), SendError<Status>> {
        match self {
            Self::Mempool(inner) => inner.send(status),
            Self::Server(inner) => inner.send(status),
            Self::DBManager(inner) => inner.send(status),
        }
    }
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        match self {
            Self::Mempool(inner) => Self::Mempool(inner.clone()),
            Self::Server(inner) => Self::Server(inner.clone()),
            Self::DBManager(inner) => Self::DBManager(inner.clone()),
        }
    }
}

#[derive(Debug)]
pub enum State {
    MempoolShutdown(TrackerError),
    ServerShutdown(TrackerError),
    DBShutdown(TrackerError),
    Healthy(String),
}

#[derive(Debug)]
pub struct Status {
    pub state: State,
}

fn send_status(sender: &Sender, e: TrackerError, outcome: ErrorBranch) -> ErrorBranch {
    match sender {
        Sender::Mempool(tx) => match e {
            TrackerError::MempoolIndexerError => {
                tx.send(Status {
                    state: State::MempoolShutdown(e),
                })
                .unwrap_or(());
            }
            _ => {
                // let string_err = e.to_string();
                tx.send(Status {
                    state: State::Healthy("error occured in mempool".to_string()),
                })
                .unwrap_or(());
            }
        },
        Sender::Server(tx) => {
            tx.send(Status {
                state: State::ServerShutdown(e),
            })
            .unwrap_or(());
        }
        Sender::DBManager(tx) => {
            tx.send(Status {
                state: State::DBShutdown(e),
            })
            .unwrap_or(());
        }
    }
    outcome
}

pub fn handle_error(sender: &Sender, e: TrackerError) -> ErrorBranch {
    match e {
        TrackerError::DbManagerExited => send_status(sender, e, ErrorBranch::Break),
        TrackerError::MempoolIndexerError => send_status(sender, e, ErrorBranch::Break),
        TrackerError::ServerError => send_status(sender, e, ErrorBranch::Break),
        TrackerError::Shutdown => send_status(sender, e, ErrorBranch::Break),
        TrackerError::IOError(_) => send_status(sender, e, ErrorBranch::Break),
        TrackerError::RPCError(_) => send_status(sender, e, ErrorBranch::Break),
    }
}
