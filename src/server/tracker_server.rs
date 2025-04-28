use mio::{Events, Interest, Poll, Token, net::TcpStream};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::sync::mpsc;
use std::sync::mpsc::Sender;

use crate::handle_result;
use crate::status;
use crate::types::DbRequest;

const SERVER: Token = Token(0);

pub fn run(db_tx: Sender<DbRequest>, status_tx: status::Sender) {
    let addr = "127.0.0.1:8080".parse().unwrap();
    let mut events = Events::with_capacity(1024);
    let mut poll = Poll::new().unwrap();
    let mut server = mio::net::TcpListener::bind(addr).expect("Port binding failed");
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)
        .expect("Couldn't register the file descriptors");

    let mut clients: HashMap<Token, TcpStream> = HashMap::new();
    let mut unique_token = 1;

    loop {
        handle_result!(status_tx, poll.poll(&mut events, None));
        let _ = events.iter().map(|e| match e.token() {
            SERVER => {
                if let Ok((mut connection, _)) = server.accept() {
                    let token = Token(unique_token);
                    unique_token += 1;
                    poll.registry()
                        .register(&mut connection, token, Interest::READABLE)
                        .unwrap();
                    clients.insert(token, connection);
                }
            }
            token => {
                if let Some(client) = clients.get_mut(&token) {
                    let mut buf = [0; 1024];
                    match client.read(&mut buf) {
                        Ok(0) => {
                            clients.remove(&token);
                        }
                        Ok(n) => {
                            let request = String::from_utf8_lossy(&buf[..n]);
                            if request.contains("GET") {
                                let (resp_tx, resp_rx) = mpsc::channel();
                                let _ = db_tx.send(DbRequest::Query("asc".to_string(), resp_tx));
                                if let Ok(server) = resp_rx.recv() {
                                    let response = format!("{:?}", server);
                                    let _ = client.write_all(response.as_bytes());
                                }
                            }
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                        Err(_) => {
                            clients.remove(&token);
                        }
                    }
                }
            }
        });
    }
}
