use crate::{error::TrackerError, handle_error::ErrorBranch};
use tokio::sync::mpsc::{self, error::SendError};

#[derive(Debug)]
pub enum Sender {
    Mempool(mpsc::Sender<Status>),
    Server(mpsc::Sender<Status>),
    DBManager(mpsc::Sender<Status>),
}

impl Sender {
    pub async fn send(&self, status: Status) -> Result<(), SendError<Status>> {
        match self {
            Self::Mempool(inner) => inner.send(status).await,
            Self::Server(inner) => inner.send(status).await,
            Self::DBManager(inner) => inner.send(status).await,
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

async fn send_status(sender: &Sender, e: TrackerError, outcome: ErrorBranch) -> ErrorBranch {
    match sender {
        Sender::Mempool(tx) => match e {
            TrackerError::MempoolIndexerError => {
                tx.send(Status {
                    state: State::MempoolShutdown(e),
                })
                .await.unwrap_or(());
            }
            _ => {
                tx.send(Status {
                    state: State::Healthy("error occured in mempool".to_string()),
                })
                .await.unwrap_or(());
            }
        },
        Sender::Server(tx) => {
            tx.send(Status {
                state: State::ServerShutdown(e),
            })
            .await.unwrap_or(());
        }
        Sender::DBManager(tx) => {
            tx.send(Status {
                state: State::DBShutdown(e),
            })
            .await.unwrap_or(());
        }
    }
    outcome
}

pub async fn handle_error(sender: &Sender, e: TrackerError) -> ErrorBranch {
    match e {
        TrackerError::DbManagerExited => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::MempoolIndexerError => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::ServerError => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::Shutdown => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::IOError(_) => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::RPCError(_) => send_status(sender, e, ErrorBranch::Break).await,
        TrackerError::ParsingError => send_status(sender, e, ErrorBranch::Continue).await,
        TrackerError::SendError => send_status(sender, e, ErrorBranch::Break).await
    }
}
