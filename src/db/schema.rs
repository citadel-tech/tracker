// @generated automatically by Diesel CLI.

diesel::table! {
    mempool_inputs (rowid) {
        rowid -> Integer,
        txid -> Text,
        input_txid -> Text,
        input_vout -> Integer,
    }
}

diesel::table! {
    mempool_tx (txid) {
        txid -> Text,
        seen_at -> Timestamp,
    }
}

diesel::table! {
    servers (onion_address) {
        onion_address -> Nullable<Text>,
        cooldown_seconds -> Float,
        stale -> Bool,
    }
}

diesel::table! {
    utxos (txid, vout) {
        txid -> Text,
        vout -> Integer,
        value -> Integer,
        script_pubkey -> Text,
        confirmed -> Bool,
        spent -> Bool,
        spent_by_txid -> Nullable<Text>,
        block_height -> Nullable<Integer>,
    }
}

diesel::joinable!(mempool_inputs -> mempool_tx (txid));

diesel::allow_tables_to_appear_in_same_query!(mempool_inputs, mempool_tx, servers, utxos,);
