use std::{sync::Arc, time::Duration};

use diesel::{SqliteConnection, r2d2::ConnectionManager};
use r2d2::Pool;
use tokio::{sync::mpsc::Sender, time::Instant};

use bitcoincore_rpc::bitcoin::absolute::{Height, LockTime};
use std::str::FromStr;
use tracing::info;

use super::rpc::BitcoinRpc;
use crate::{
    handle_result,
    indexer::utxo_indexer::Indexer,
    status,
    types::{DbRequest, ServerInfo},
};

pub async fn run(
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    db_tx: Sender<DbRequest>,
    status_tx: status::Sender,
    client: BitcoinRpc,
) {
    info!("Indexer started");
    let mut utxo_indexer = Indexer::new(pool, &client);
    let mut last_tip = 0;
    loop {
        let blockchain_info = handle_result!(status_tx, client.get_blockchain_info());
        let tip_height = blockchain_info.blocks + 1;
        utxo_indexer.process_mempool();

        for height in last_tip..tip_height {
            utxo_indexer.process_block(height);
            let block_hash = handle_result!(status_tx, client.get_block_hash(height));
            let block = handle_result!(status_tx, client.get_block(block_hash));
            for tx in block.txdata {
                if tx.lock_time == LockTime::Blocks(Height::ZERO) {
                    continue;
                }

                if tx.output.len() < 2 || tx.output.len() > 5 {
                    continue;
                }

                let onion_address = tx.output.iter().find_map(|txout| {
                    extract_onion_address_from_script(txout.script_pubkey.as_bytes())
                });

                if let Some(onion_address) = onion_address {
                    let server_info = ServerInfo {
                        onion_address: onion_address.clone(),
                        cooldown: Instant::now(),
                        stale: false,
                    };
                    info!("New address found: {:?}", onion_address);
                    let db_request = DbRequest::Add(onion_address, server_info);

                    handle_result!(status_tx, db_tx.send(db_request).await);
                }
            }
        }
        last_tip = tip_height;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

fn extract_onion_address_from_script(script: &[u8]) -> Option<String> {
    if script.is_empty() || script[0] != 0x6a {
        return None;
    }
    if script.len() < 2 {
        return None;
    }
    let (data_start, data_len) = match script[1] {
        n @ 0x01..=0x4b => (2, n as usize),
        0x4c => {
            if script.len() < 3 {
                return None;
            }
            (3, script[2] as usize)
        }
        0x4d => {
            if script.len() < 4 {
                return None;
            }
            let len = u16::from_le_bytes([script[2], script[3]]) as usize;
            (4, len)
        }
        _ => {
            return None;
        }
    };
    if script.len() < data_start + data_len {
        return None;
    }

    let data = &script[data_start..data_start + data_len];
    let decoded = String::from_utf8(data.to_vec()).ok()?;

    #[cfg(not(feature = "integration-test"))]
    if is_valid_onion_address(&decoded) {
        Some(decoded)
    } else {
        None
    }

    #[cfg(feature = "integration-test")]
    if is_valid_address(&decoded) {
        Some(decoded)
    } else {
        None
    }
}

#[cfg(not(feature = "integration-test"))]
fn is_valid_onion_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    let domain = parts[0];
    let port = parts[1];
    if !domain.ends_with(".onion") {
        return false;
    }
    matches!(port.parse::<u16>(), Ok(p) if p > 0)
}

#[cfg(feature = "integration-test")]
fn is_valid_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return false;
    }

    let ip = parts[0];
    let port = parts[1];

    if std::net::Ipv4Addr::from_str(ip).is_err() {
        return false;
    }

    matches!(port.parse::<u16>(), Ok(p) if p > 0)
}

#[cfg(not(feature = "integration-test"))]
#[cfg(test)]
mod tests_onion {
    use super::*;

    #[test]
    fn test_valid_onion_address() {
        assert!(is_valid_onion_address("example.onion:1234"));
        assert!(is_valid_onion_address("abc1234567890def.onion:65535"));
    }

    #[test]
    fn test_invalid_onion_address() {
        assert!(!is_valid_onion_address("example.com:1234"));
        assert!(!is_valid_onion_address("example.onion:0"));
        assert!(!is_valid_onion_address("example.onion"));
        assert!(!is_valid_onion_address("127.0.0.1:8080"));
    }
}

#[cfg(feature = "integration-test")]
#[cfg(test)]
mod tests_ipv4 {
    use super::*;

    #[test]
    fn test_valid_ipv4_address() {
        assert!(is_valid_address("127.0.0.1:8080"));
        assert!(is_valid_address("192.168.1.1:65535"));
    }

    #[test]
    fn test_invalid_ipv4_address() {
        assert!(!is_valid_address("example.onion:1234"));
        assert!(!is_valid_address("256.0.0.1:8080"));
        assert!(!is_valid_address("127.0.0.1:0"));
        assert!(!is_valid_address("127.0.0.1"));
        assert!(!is_valid_address("::1:8080"));
    }
}
