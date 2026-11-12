use std::{collections::HashMap, sync::Arc};

use lilac_proto::ServerMsg;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

/// Per-connection capacity for outbound WebSocket frames (prevents OOM on slow clients).
pub const WS_OUTBOUND_BUFFER: usize = 64;

/// Thread-safe registry mapping `user_id` -> active `(conn_id, bounded_sender)` sockets.
///
/// Keying each socket by `conn_id` ensures closing an older browser tab never
/// disconnects a newer tab for the same user.
#[derive(Debug, Default)]
pub struct ConnectionRegistry {
    connections: RwLock<HashMap<Uuid, Vec<(Uuid, mpsc::Sender<ServerMsg>)>>>,
}

impl ConnectionRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub async fn register(
        &self,
        user_id: Uuid,
        conn_id: Uuid,
        tx: mpsc::Sender<ServerMsg>,
    ) {
        let mut guard = self.connections.write().await;
        guard.entry(user_id).or_default().push((conn_id, tx));
    }

    pub async fn unregister(&self, user_id: Uuid, conn_id: Uuid) {
        let mut guard = self.connections.write().await;
        if let Some(list) = guard.get_mut(&user_id) {
            list.retain(|(id, _)| *id != conn_id);
            if list.is_empty() {
                guard.remove(&user_id);
            }
        }
    }

    /// Sends a [`ServerMsg`] to all active sockets of `user_id`; prunes closed sockets.
    /// Returns `true` if at least one socket accepted the message.
    pub async fn send_to_user(&self, user_id: Uuid, msg: ServerMsg) -> bool {
        let senders: Vec<(Uuid, mpsc::Sender<ServerMsg>)> = {
            let guard = self.connections.read().await;
            match guard.get(&user_id) {
                Some(list) => list.clone(),
                None => return false,
            }
        };

        let mut delivered = false;
        let mut stale_ids = Vec::new();
        for (conn_id, tx) in senders {
            match tx.try_send(msg.clone()) {
                Ok(()) => delivered = true,
                Err(mpsc::error::TrySendError::Closed(_)) => stale_ids.push(conn_id),
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!(%user_id, %conn_id, "dropping frame for slow websocket client");
                    stale_ids.push(conn_id);
                }
            }
        }

        if !stale_ids.is_empty() {
            let mut guard = self.connections.write().await;
            if let Some(list) = guard.get_mut(&user_id) {
                list.retain(|(id, _)| !stale_ids.contains(id));
                if list.is_empty() {
                    guard.remove(&user_id);
                }
            }
        }
        delivered
    }

    pub async fn online_count(&self) -> usize {
        self.connections.read().await.len()
    }
}
