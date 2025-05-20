use bitcoincore_rpc::{
    Auth, Client, RpcApi,
    bitcoin::{Block, BlockHash, Transaction, Txid},
    json::GetBlockchainInfoResult,
};

use crate::error::TrackerError;

pub struct BitcoinRpc {
    client: Client,
}

impl BitcoinRpc {
    pub fn new(url: String, username: String, password: String) -> Result<Self, TrackerError> {
        let auth = Auth::UserPass(username, password);
        let client = Client::new(&url, auth)?;
        Ok(Self { client })
    }

    pub fn get_raw_mempool(&self) -> Result<Vec<Txid>, TrackerError> {
        let raw_mempool = self.client.get_raw_mempool()?;
        Ok(raw_mempool)
    }

    pub fn get_raw_tx(&self, txid: &Txid) -> Result<Transaction, TrackerError> {
        let tx = self.client.get_raw_transaction(txid, None)?;
        Ok(tx)
    }

    pub fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, TrackerError> {
        let blockchain_info = self.client.get_blockchain_info()?;
        Ok(blockchain_info)
    }

    pub fn get_block_hash(&self, height: u64) -> Result<BlockHash, TrackerError> {
        let block_hash = self.client.get_block_hash(height)?;
        Ok(block_hash)
    }

    pub fn get_block(&self, hash: BlockHash) -> Result<Block, TrackerError> {
        let block = self.client.get_block(&hash)?;
        Ok(block)
    }
}

impl From<Client> for BitcoinRpc {
    fn from(value: Client) -> Self {
        BitcoinRpc { client: value }
    }
}
