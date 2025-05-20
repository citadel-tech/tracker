#![allow(dead_code)]
use std::path::Path;

use bitcoincore_rpc::Auth;
use bitcoincore_rpc::Client;
use clap::Parser;
use error::TrackerError;
use status::{State, Status};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tor::check_tor_status;
use tor::get_tor_hostname;
use tracing::error;
use tracing::{info, warn};
use types::DbRequest;
mod db;
mod error;
mod handle_error;
mod indexer;
mod server;
mod status;
mod tor;
mod types;
mod utils;

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
        default_value = "127.0.0.1:8080"
    )]
    pub address: String,

    #[clap(name = "control port PORT", short = 'c', long, default_value = "9051")]
    pub control_port: u16,

    #[clap(name = "tor_auth_password", long, default_value = "")]
    pub tor_auth_password: String,

    #[clap(name = "socks port PORT", long, default_value = "9050")]
    pub socks_port: u16,

    #[clap(name = "datadir", long, default_value = ".tracker")]
    pub datadir: String,
}

fn parse_proxy_auth(s: &str) -> Result<(String, String), TrackerError> {
    let parts: Vec<_> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(TrackerError::ParsingError);
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
        RPCConfig { url, auth }
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
        Client::new(&value.url, value.auth.clone()).unwrap()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = App::parse();

    let rpc_config = RPCConfig::new(args.rpc, Auth::UserPass(args.auth.0, args.auth.1));

    check_tor_status(args.control_port, &args.tor_auth_password)
        .await
        .expect("Failed to check Tor status");

    let hostname = match args.address.split_once(':') {
        Some((_, port)) => {
            let port = port.parse::<u16>().expect("Invalid port in address");
            get_tor_hostname(
                Path::new(&args.datadir),
                args.control_port,
                port,
                &args.tor_auth_password,
            )
            .await
            .expect("Failed to retrieve Tor hostname")
        }
        None => {
            error!("Invalid address format. Expected format: <host>:<port>");
            return;
        }
    };

    info!("Tracker is listening at {}", hostname);

    let (mut db_tx, db_rx) = mpsc::channel::<DbRequest>(10);
    let (status_tx, mut status_rx) = mpsc::channel::<Status>(10);

    let server_address = args.address.clone();

    spawn_db_manager(db_rx, status_tx.clone()).await;
    spawn_mempool_indexer(db_tx.clone(), status_tx.clone(), rpc_config.clone().into()).await;
    spawn_server(
        db_tx.clone(),
        status_tx.clone(),
        server_address.clone(),
        args.socks_port,
    )
    .await;

    info!("Tracker started");

    while let Some(status) = status_rx.recv().await {
        match status.state {
            State::DBShutdown(err) => {
                warn!(
                    "DB Manager exited unexpectedly. Restarting... Error: {:?}",
                    err
                );
                let (new_db_tx, new_db_rx) = mpsc::channel::<DbRequest>(10);
                db_tx = new_db_tx;
                spawn_db_manager(new_db_rx, status_tx.clone()).await;
            }
            State::Healthy(info) => {
                info!("System healthy: {:?}", info);
            }
            State::MempoolShutdown(err) => {
                warn!("Mempool Indexer crashed. Restarting... Error: {:?}", err);
                let client: Client = rpc_config.clone().into();
                spawn_mempool_indexer(db_tx.clone(), status_tx.clone(), client).await;
            }
            State::ServerShutdown(err) => {
                warn!("Server crashed. Restarting... Error: {:?}", err);
                spawn_server(
                    db_tx.clone(),
                    status_tx.clone(),
                    server_address.clone(),
                    args.socks_port,
                )
                .await;
            }
        }
    }
}

async fn spawn_db_manager(db_tx: Receiver<DbRequest>, status_tx: Sender<Status>) {
    info!("Spawning db manager");
    tokio::spawn(db::run(db_tx, status::Sender::DBManager(status_tx)));
}

async fn spawn_mempool_indexer(
    db_tx: Sender<DbRequest>,
    status_tx: Sender<Status>,
    client: Client,
) {
    info!("Spawning indexer");
    tokio::spawn(indexer::run(
        db_tx,
        status::Sender::Mempool(status_tx),
        client.into(),
    ));
}

async fn spawn_server(
    db_tx: Sender<DbRequest>,
    status_tx: Sender<Status>,
    address: String,
    socks_port: u16,
) {
    info!("Spawning server instance");
    tokio::spawn(server::run(
        db_tx,
        status::Sender::Server(status_tx),
        address,
        socks_port,
    ));
}
