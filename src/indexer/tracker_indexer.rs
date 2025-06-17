use std::time::Duration;

use tokio::{sync::mpsc::Sender, time::Instant};

use bitcoincore_rpc::bitcoin::absolute::{Height, LockTime};
use tracing::info;

use super::rpc::BitcoinRpc;
use crate::{
    handle_result, status,
    types::{DbRequest, ServerInfo},
};

pub async fn run(db_tx: Sender<DbRequest>, status_tx: status::Sender, client: BitcoinRpc) {
    info!("Indexer started");
    let mut last_tip = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let blockchain_info = handle_result!(status_tx, client.get_blockchain_info());
        let tip_height = blockchain_info.blocks;
        for height in last_tip..tip_height {
            let block_hash = handle_result!(status_tx, client.get_block_hash(height));
            let block = handle_result!(status_tx, client.get_block(block_hash));
            for tx in block.txdata {
                if tx.lock_time == LockTime::Blocks(Height::ZERO) {
                    continue;
                }
                if tx.output.len() != 2 {
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
    if is_valid_onion_address(&decoded) {
        Some(decoded)
    } else {
        None
    }
}

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
