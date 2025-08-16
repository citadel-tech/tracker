use std::collections::HashMap;

use bitcoincore_rpc::bitcoin::OutPoint;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::types::{SubscriptionInfo, UtxoSpentNotification};

pub struct SubscriptionManager {
    subscriptions: HashMap<OutPoint, Vec<SubscriptionInfo>>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    pub fn add_subscription(
        &mut self,
        outpoint: OutPoint,
        client_id: String,
        notification_tx: mpsc::Sender<UtxoSpentNotification>,
    ) {
        let sub = SubscriptionInfo {
            client_id: client_id.clone(),
            outpoint,
            subscribed_at: tokio::time::Instant::now(),
            connection_tx: notification_tx,
        };

        self.subscriptions
            .entry(outpoint)
            .or_insert_with(Default::default)
            .push(sub);

        info!(
            "add subscription for client {} to UTXO {:?}",
            client_id, outpoint
        );
    }

    pub fn remove_subscription(&mut self, outpoint: OutPoint, client_id: String) -> bool {
        if let Some(subscribers) = self.subscriptions.clone().get_mut(&outpoint) {
            let init_len = subscribers.len();
            subscribers.retain(|sub| sub.client_id != client_id);

            if subscribers.is_empty() {
                self.subscriptions.remove(&outpoint);
            }

            let is_removed = init_len != subscribers.len();
            if is_removed {
                info!(
                    "removed subscription for client {} from UTXO {:?}",
                    client_id, outpoint
                );
            }
            return is_removed;
        }
        false
    }

    pub async fn notify(&mut self, outpoint: OutPoint, notification: UtxoSpentNotification) {
        if let Some(subscribers) = self.subscriptions.get_mut(&outpoint) {
            info!(
                "Notifying {} subscribers about UTXO {:?} being spent...",
                subscribers.len(),
                outpoint
            );

            let mut failed_clients = vec![];

            for sub in subscribers.iter() {
                match sub.connection_tx.try_send(notification.clone()) {
                    Ok(_) => {
                        info!("notified client {} about UTXO spending", sub.client_id);
                    }

                    Err(e) => {
                        warn!("Failed to notify client {}: {:?}", sub.client_id, e);
                        failed_clients.push(sub.client_id.clone());
                    }
                }
            }

            for failed_client in failed_clients {
                self.remove_subscription(outpoint, failed_client);
            }
        }
    }

    pub fn get_subscription_count(&self, outpoint: &OutPoint) -> usize {
        self.subscriptions
            .get(outpoint)
            .map_or(0, |subscriptions| subscriptions.len())
    }

    pub fn get_total_subscriptions(&self) -> usize {
        self.subscriptions
            .values()
            .map(|subscriptions| subscriptions.len())
            .sum()
    }

    pub fn get_subscriptions(&self, outpoint: OutPoint) -> Vec<SubscriptionInfo> {
        self.subscriptions
            .get(&outpoint)
            .unwrap_or(&Vec::new())
            .to_vec()
    }
}
