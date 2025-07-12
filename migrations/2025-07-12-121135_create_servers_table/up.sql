-- Your SQL goes here
CREATE TABLE servers (
    onion_address TEXT PRIMARY KEY,
    cooldown_seconds REAL NOT NULL,
    stale BOOLEAN NOT NULL
);

CREATE TABLE utxos (
    txid TEXT NOT NULL,
    vout INTEGER NOT NULL,
    value INTEGER NOT NULL,
    script_pubkey TEXT NOT NULL,
    confirmed BOOLEAN NOT NULL DEFAULT true,
    spent BOOLEAN NOT NULL DEFAULT false,
    spent_by_txid TEXT,
    block_height INTEGER,
    PRIMARY KEY (txid, vout)
);


CREATE TABLE mempool_tx (
    txid TEXT PRIMARY KEY,
    seen_at TIMESTAMP NOT NULL
);

CREATE TABLE mempool_inputs (
    txid TEXT NOT NULL,
    input_txid TEXT NOT NULL,
    input_vout INTEGER NOT NULL,
    FOREIGN KEY(txid) REFERENCES mempool_tx(txid),
    FOREIGN KEY(input_txid, input_vout) REFERENCES utxos(txid, vout)
);