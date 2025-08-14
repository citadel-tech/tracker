#![allow(warnings)]
use bitcoincore_rpc::{Auth, Client};
use diesel::SqliteConnection;
use diesel::r2d2::ConnectionManager;
use r2d2::Pool;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::status::{State, Status};
use crate::types::DbRequest;

mod db;
mod error;
mod handle_error;
mod indexer;
mod server;
mod status;
mod tor;
mod types;
mod utils;

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[cfg(not(feature = "integration-test"))]
#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub rpc_auth: Auth,
    pub address: String,
    pub control_port: u16,
    pub tor_auth_password: String,
    pub socks_port: u16,
    pub datadir: String,
}

#[cfg(feature = "integration-test")]
#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub rpc_auth: Auth,
    pub address: String,
    pub datadir: String,
}

fn run_migrations(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>) {
    let mut conn = pool
        .get()
        .expect("Failed to get DB connection for migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Migration failed");
}

pub async fn start(cfg: Config) {
    info!("Connecting to indexer db");
    let database_url = format!("{}/tracker.db", cfg.datadir);
    if let Some(parent) = Path::new(&database_url).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create database directory");
    }
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Arc::new(
        Pool::builder()
            .build(manager)
            .expect("Failed to create DB pool"),
    );
    run_migrations(pool.clone());
    info!("Connected to indexer db");

    #[cfg(not(feature = "integration-test"))]
    tor::check_tor_status(cfg.control_port, &cfg.tor_auth_password)
        .await
        .expect("Failed to check Tor status");

    #[cfg(not(feature = "integration-test"))]
    let hostname = match cfg.address.split_once(':') {
        Some((_, port)) => {
            let port = port.parse::<u16>().expect("Invalid port in address");
            tor::get_tor_hostname(
                Path::new(&cfg.datadir),
                cfg.control_port,
                port,
                &cfg.tor_auth_password,
            )
            .await
            .expect("Failed to retrieve Tor hostname")
        }
        None => {
            error!("Invalid address format. Expected format: <host>:<port>");
            return;
        }
    };

    #[cfg(feature = "integration-test")]
    let hostname = cfg.address.clone();

    info!("Tracker is listening at {}", hostname);

    let (mut db_tx, db_rx) = mpsc::channel::<DbRequest>(10);
    let (status_tx, mut status_rx) = mpsc::channel::<Status>(10);

    let rpc_client = Client::new(&cfg.rpc_url, cfg.rpc_auth.clone()).unwrap();

    spawn_db_manager(pool.clone(), db_rx, status_tx.clone()).await;
    spawn_mempool_indexer(pool.clone(), db_tx.clone(), status_tx.clone(), rpc_client).await;
    spawn_server(
        db_tx.clone(),
        status_tx.clone(),
        cfg.address.clone(),
        #[cfg(not(feature = "integration-test"))]
        cfg.socks_port,
        hostname.clone(),
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
                spawn_db_manager(pool.clone(), new_db_rx, status_tx.clone()).await;
            }
            State::Healthy(info) => {
                info!("System healthy: {:?}", info);
            }
            State::MempoolShutdown(err) => {
                warn!("Mempool Indexer crashed. Restarting... Error: {:?}", err);
                let client = Client::new(&cfg.rpc_url, cfg.rpc_auth.clone()).unwrap();
                spawn_mempool_indexer(pool.clone(), db_tx.clone(), status_tx.clone(), client).await;
            }
            State::ServerShutdown(err) => {
                warn!("Server crashed. Restarting... Error: {:?}", err);
                spawn_server(
                    db_tx.clone(),
                    status_tx.clone(),
                    cfg.address.clone(),
                    #[cfg(not(feature = "integration-test"))]
                    cfg.socks_port,
                    hostname.clone(),
                )
                .await;
            }
        }
    }
}

async fn spawn_db_manager(
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    db_rx: tokio::sync::mpsc::Receiver<DbRequest>,
    status_tx: tokio::sync::mpsc::Sender<Status>,
) {
    info!("Spawning db manager");
    tokio::spawn(db::run(pool, db_rx, status::Sender::DBManager(status_tx)));
}

async fn spawn_mempool_indexer(
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    db_tx: tokio::sync::mpsc::Sender<DbRequest>,
    status_tx: tokio::sync::mpsc::Sender<Status>,
    client: Client,
) {
    info!("Spawning indexer");
    tokio::spawn(indexer::run(
        pool,
        db_tx,
        status::Sender::Mempool(status_tx),
        client.into(),
    ));
}

async fn spawn_server(
    db_tx: tokio::sync::mpsc::Sender<DbRequest>,
    status_tx: tokio::sync::mpsc::Sender<Status>,
    address: String,
    #[cfg(not(feature = "integration-test"))] socks_port: u16,
    hostname: String,
) {
    info!("Spawning server instance");
    tokio::spawn(server::run(
        db_tx,
        status::Sender::Server(status_tx),
        address,
        #[cfg(not(feature = "integration-test"))]
        socks_port,
        hostname,
    ));
}
