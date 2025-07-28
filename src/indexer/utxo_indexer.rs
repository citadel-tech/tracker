use std::sync::Arc;

use crate::db::model::{MempoolInput, MempoolTx, Utxo};
use crate::db::schema::{mempool_inputs, mempool_tx, utxos};
use crate::indexer::rpc::BitcoinRpc;
use chrono::NaiveDateTime;
use diesel::SqliteConnection;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use r2d2::Pool;

pub struct Indexer<'a> {
    conn: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    rpc: &'a BitcoinRpc,
}

impl<'a> Indexer<'a> {
    pub fn new(conn: Arc<Pool<ConnectionManager<SqliteConnection>>>, rpc: &'a BitcoinRpc) -> Self {
        Self { conn, rpc }
    }

    pub fn process_mempool(&mut self) {
        let txids = self.rpc.get_raw_mempool().unwrap();
        let mut conn = self.conn.get().expect("Failed to get DB connection");

        for txid in txids {
            let tx = self.rpc.get_raw_tx(&txid).unwrap();

            self.insert_mempool_tx(&txid.to_string());

            for input in &tx.input {
                let prevout = &input.previous_output;
                diesel::insert_into(mempool_inputs::table)
                    .values(&MempoolInput {
                        txid: txid.to_string(),
                        input_txid: prevout.txid.to_string(),
                        input_vout: prevout.vout as i32,
                    })
                    .execute(&mut conn)
                    .unwrap();

                self.mark_utxo_spent(
                    &txid.to_string(),
                    prevout.vout as i32,
                    Some(&txid.to_string()),
                    false,
                );
            }

            for (vout, out) in tx.output.iter().enumerate() {
                let utxo = Utxo {
                    txid: txid.to_string().clone(),
                    vout: vout as i32,
                    value: out.value.to_sat() as i32,
                    script_pubkey: out.script_pubkey.to_hex_string(),
                    confirmed: false,
                    spent: false,
                    spent_by_txid: None,
                    block_height: None,
                };
                diesel::insert_or_ignore_into(utxos::table)
                    .values(&utxo)
                    .execute(&mut conn)
                    .unwrap();
            }
        }
    }

    pub fn process_block(&mut self, height: u64) {
        let block_hash = self.rpc.get_block_hash(height).unwrap();
        let block = self.rpc.get_block(block_hash).unwrap();
        let mut conn = self.conn.get().expect("Failed to get DB connection");

        for tx in block.txdata.iter() {
            for input in &tx.input {
                let prevout = &input.previous_output;
                self.mark_utxo_spent(
                    &prevout.txid.to_string(),
                    prevout.vout as i32,
                    Some(&tx.compute_txid().to_string()),
                    true,
                );
            }

            for (vout, out) in tx.output.iter().enumerate() {
                let utxo = Utxo {
                    txid: tx.compute_txid().to_string(),
                    vout: vout as i32,
                    value: out.value.to_sat() as i32,
                    script_pubkey: out.script_pubkey.to_hex_string(),
                    confirmed: true,
                    spent: false,
                    spent_by_txid: None,
                    block_height: Some(height as i32),
                };
                diesel::insert_or_ignore_into(utxos::table)
                    .values(&utxo)
                    .execute(&mut conn)
                    .unwrap();
            }
        }
    }

    fn insert_mempool_tx(&mut self, txid: &str) {
        let mut conn = self.conn.get().expect("Failed to get DB connection");
        diesel::insert_or_ignore_into(mempool_tx::table)
            .values(&MempoolTx {
                txid: txid.to_string(),
                seen_at: NaiveDateTime::MIN,
            })
            .execute(&mut conn)
            .unwrap();
    }

    fn mark_utxo_spent(
        &mut self,
        txid: &str,
        vout: i32,
        spent_by: Option<&str>,
        is_confirmed_spend: bool,
    ) {
        use utxos::dsl;
        let mut conn = self.conn.get().expect("Failed to get DB connection");

        let confirmed_value = is_confirmed_spend;

        diesel::update(dsl::utxos.filter(dsl::txid.eq(txid).and(dsl::vout.eq(vout))))
            .set((
                dsl::spent.eq(true),
                dsl::spent_by_txid.eq(spent_by.map(str::to_string)),
                dsl::confirmed.eq(confirmed_value),
            ))
            .execute(&mut conn)
            .unwrap();
    }
}
