#![allow(dead_code)]
use status::{State, Status};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use types::DbRequest;
mod db;
mod error;
mod handle_error;
mod indexer;
mod server;
mod status;
mod types;

fn main() {
    let (mut db_tx, db_rx) = mpsc::channel::<DbRequest>();
    let (status_tx, status_rx) = mpsc::channel::<Status>();

    spawn_db_manager(db_rx, status_tx.clone());
    spawn_mempool_indexer(db_tx.clone(), status_tx.clone());
    spawn_server(db_tx.clone(), status_tx.clone());

    while let Ok(status) = status_rx.recv() {
        match status.state {
            State::DBShutdown(e) => {
                println!("DB Manager exited. Restarting..., {:?}", e);
                let (db_tx_s, db_rx) = mpsc::channel::<DbRequest>();
                db_tx = db_tx_s;
                spawn_db_manager(db_rx, status_tx.clone());
            }
            State::Healthy(e) => {
                println!("All looks good: {:?}", e);
            }
            State::MempoolShutdown(e) => {
                println!(
                    "Mempool Indexer encountered an error. Restarting...: {:?}",
                    e
                );
                spawn_mempool_indexer(db_tx.clone(), status_tx.clone());
            }
            State::ServerShutdown(e) => {
                println!("Server encountered an error. Restarting...: {:?}", e);
                spawn_server(db_tx.clone(), status_tx.clone());
            }
        }
    }
}

fn spawn_db_manager(db_tx: Receiver<DbRequest>, status_tx: Sender<Status>) {
    thread::spawn(move || {
        db::run(db_tx, status::Sender::DBManager(status_tx));
    });
}

fn spawn_mempool_indexer(db_tx: Sender<DbRequest>, status_tx: Sender<Status>) {
    thread::spawn(move || {
        indexer::run(
            db_tx,
            status::Sender::Mempool(status_tx),
            "user".to_string(),
            "password".to_string(),
            "Something".to_string(),
        );
    });
}

fn spawn_server(db_tx: Sender<DbRequest>, status_tx: Sender<Status>) {
    thread::spawn(move || {
        server::run(db_tx, status::Sender::Server(status_tx));
    });
}
