use bitcoincore_rpc::bitcoin::{
    Amount, OutPoint, PublicKey, absolute::LockTime, hashes::hash160::Hash,
    secp256k1::ecdsa::Signature,
};
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::Sender, time::Instant};

use crate::db::model::MempoolTx;

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub onion_address: String,
    pub cooldown: Instant,
    pub stale: bool,
}

pub enum DbRequest {
    Add(String, ServerInfo),
    Query(String, Sender<Option<ServerInfo>>),
    Update(String, ServerInfo),
    QueryAll(Sender<Vec<(String, ServerInfo)>>),
    QueryActive(Sender<Vec<String>>),
    WatchUtxo(OutPoint, Sender<Vec<MempoolTx>>),
    AddSubscription(
        OutPoint,
        String,
        tokio::sync::mpsc::Sender<UtxoSpentNotification>,
    ),
    RemoveSubscription(OutPoint, String),
    GetSubscriptions(OutPoint, tokio::sync::mpsc::Sender<Vec<SubscriptionInfo>>),
    NotifyUtxoSpent(OutPoint, UtxoSpentNotification),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Hash)]
pub struct FidelityBond {
    pub(crate) outpoint: OutPoint,
    /// Fidelity Amount
    pub amount: Amount,
    /// Fidelity Locktime
    pub lock_time: LockTime,
    pub(crate) pubkey: PublicKey,
    // Height at which the bond was confirmed.
    pub(crate) conf_height: Option<u32>,
    // Cert expiry denoted in multiple of difficulty adjustment period (2016 blocks)
    pub(crate) cert_expiry: Option<u32>,
}

/// Contains proof data related to fidelity bond.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FidelityProof {
    pub(crate) bond: FidelityBond,
    pub(crate) cert_hash: Hash,
    pub(crate) cert_sig: Signature,
}

/// Metadata shared by the maker with the Directory Server for verifying authenticity.
#[derive(Serialize, Deserialize, Debug)]
#[allow(private_interfaces)]
pub struct DnsMetadata {
    /// The maker's URL.
    pub url: String,
    /// Proof of the maker's fidelity bond funding.
    pub proof: FidelityProof,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UtxoSpentNotification {
    pub watched_outpoint: OutPoint,
    pub spending_txid: String,
    pub spending_input_index: u32,
    pub block_height: Option<u32>,
    pub confirmed: bool,
    pub timestamp: chrono::NaiveDateTime,
}

#[derive(Clone, Debug)]
pub struct SubscriptionInfo {
    pub client_id: String,
    pub outpoint: OutPoint,
    pub subscribed_at: Instant,
    pub connection_tx: tokio::sync::mpsc::Sender<UtxoSpentNotification>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TrackerClientToServer {
    /// A request sent by the maker to register itself with the DNS server and authenticate.
    Post {
        /// Metadata containing the maker's URL and fidelity proof.
        metadata: DnsMetadata,
    },
    /// A request sent by the taker to fetch all valid maker addresses from the DNS server.
    Get,
    /// To gauge server activity
    Pong {
        address: String,
    },
    Watch {
        outpoint: OutPoint,
    },
    /// Subscribe to UTXO spending notifications
    Subscribe {
        outpoint: OutPoint,
        client_id: String,
    },

    /// Unsubscribe from UTXO not/ifications
    Unsubscribe {
        outpoint: OutPoint,
        client_id: String,
    },

    /// Keep connection alive
    Heartbeat,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TrackerServerToClient {
    Address { addresses: Vec<String> },
    Ping { address: String, port: u16 },
    WatchResponse { mempool_tx: Vec<MempoolTx> },
    UtxoSpent(UtxoSpentNotification),

    SubscriptionConfirmed { outpoint: OutPoint },

    SubscriptionRemoved { outpoint: OutPoint },

    HeartbeatAck,
}
