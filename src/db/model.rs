use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::db::schema::servers)]
pub struct Server {
    pub onion_address: String,
    pub cooldown_seconds: f32,
    pub stale: bool,
}

#[derive(Queryable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::db::schema::utxos)]
pub struct Utxo {
    pub txid: String,
    pub vout: i32,
    pub value: i32,
    pub script_pubkey: String,
    pub confirmed: bool,
    pub spent: bool,
    pub spent_by_txid: Option<String>,
    pub block_height: Option<i32>,
}

#[derive(Queryable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::db::schema::mempool_tx)]
pub struct MempoolTx {
    pub txid: String,
    pub seen_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::db::schema::mempool_inputs)]
pub struct MempoolInput {
    pub txid: String,
    pub input_txid: String,
    pub input_vout: i32,
}
