#![allow(dead_code)]
use error::TrackerError;
use status::{State, Status};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    
};
use bitcoincore_rpc::Client;
use bitcoincore_rpc::Auth;
use tracing::{info, warn};
use types::DbRequest;
use clap::Parser;
mod db;
mod error;
mod handle_error;
mod indexer;
mod server;
mod status;
mod types;


#[derive(Parser)]
#[clap(version = option_env ! ("CARGO_PKG_VERSION").unwrap_or("unknown"),
author = option_env ! ("CARGO_PKG_AUTHORS").unwrap_or(""))]
struct App {
    #[clap(
        name = "ADDRESS:PORT",
        long,
        short = 'r',
        default_value = "127.0.0.1:48332"
    )]
    pub(crate) rpc: String,
    #[clap(
        name = "USER:PASSWORD",
        short = 'a',
        long,
        value_parser = parse_proxy_auth,
        default_value = "username:password",
    )]
    pub auth: (String, String),
    #[clap(
        name = "Server ADDRESS:PORT",
        short = 's',
        long,
        default_value = "127.0.0.1:8080",
    )]
    pub address: String
}

fn parse_proxy_auth(s: &str) -> Result<(String, String), TrackerError> {
    let parts: Vec<_> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(TrackerError::ParsingError)
    }

    let user = parts[0].to_string();
    let passwd = parts[1].to_string();

    Ok((user, passwd))
}


#[derive(Debug, Clone)]
pub struct RPCConfig {
    pub url: String,
    pub auth: Auth,
}

const RPC_HOSTPORT: &str = "localhost:18443";

impl RPCConfig {
    fn new(url: String, auth: Auth) -> Self {
        RPCConfig { url, auth}
    }
}

impl Default for RPCConfig {
    fn default() -> Self {
        Self {
            url: RPC_HOSTPORT.to_string(),
            auth: Auth::UserPass("regtestrpcuser".to_string(), "regtestrpcpass".to_string()),
        }
    }
}

impl From<RPCConfig> for Client {
    fn from(value: RPCConfig) -> Self {
        let rpc = Client::new(&value.url, value.auth.clone()).unwrap();
        rpc
    }
}


#[tokio::main]
async fn main() {

    tracing_subscriber::fmt::init();

    let args = App::parse();

    let rpc_config  = RPCConfig::new(args.rpc, Auth::UserPass(args.auth.0, args.auth.1));


    let (mut db_tx, db_rx) = mpsc::channel::<DbRequest>(10);
    let (status_tx, mut status_rx) = mpsc::channel::<Status>(10);
    let server_address = args.address;

    spawn_db_manager(db_rx, status_tx.clone()).await;
    spawn_mempool_indexer(db_tx.clone(), status_tx.clone(), rpc_config.clone().into()).await;
    spawn_server(db_tx.clone(), status_tx.clone(), server_address.clone()).await;

    info!("Tracker started");

    while let Some(status) = status_rx.recv().await {
        match status.state {
            State::DBShutdown(e) => {
                warn!("DB Manager exited. Restarting..., {:?}", e);
                let (db_tx_s, db_rx) = mpsc::channel::<DbRequest>(10);
                db_tx = db_tx_s;
                spawn_db_manager(db_rx, status_tx.clone()).await;
            }
            State::Healthy(e) => {
                info!("All looks good: {:?}", e);
            }
            State::MempoolShutdown(e) => {
                warn!(
                    "Mempool Indexer encountered an error. Restarting...: {:?}",
                    e
                );
                let client: Client  = rpc_config.clone().into();
                spawn_mempool_indexer(db_tx.clone(), status_tx.clone(), client.into()).await;
            }
            State::ServerShutdown(e) => {
                warn!("Server encountered an error. Restarting...: {:?}", e);
                spawn_server(db_tx.clone(), status_tx.clone(), server_address.clone()).await;
            }
        }
    }
}

async fn spawn_db_manager(db_tx: Receiver<DbRequest>, status_tx: Sender<Status>) {
    info!("Spawning db manager");
    tokio::spawn(db::run(db_tx, status::Sender::DBManager(status_tx)));
}

async fn spawn_mempool_indexer(db_tx: Sender<DbRequest>, status_tx: Sender<Status>, client: Client) {
    info!("Spawning indexer");
    tokio::spawn(
        indexer::run(
            db_tx,
            status::Sender::Mempool(status_tx),
            client.into()
        ));
}

async fn spawn_server(db_tx: Sender<DbRequest>, status_tx: Sender<Status>, address: String) {
    info!("Spawning server instance");
    tokio::spawn(server::run(db_tx, status::Sender::Server(status_tx), address));
}
