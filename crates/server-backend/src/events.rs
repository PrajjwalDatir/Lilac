use std::sync::{Arc, Mutex};

use lilac_proto::DomainEvent;
use tokio::sync::{broadcast, mpsc};

/// Decoupled internal event bus connecting system components.
///
/// Provides both:
/// - `subscribe()`: broadcast stream for non-critical live listeners.
/// - `publish_reliable()` / `take_reliable_rx()`: backpressured `mpsc` channel that
///   never drops state-mutating events (`GameCompleted`, `BountyClaimed`) on slow consumers.
pub trait EventBus: Send + Sync {
    fn publish(&self, event: DomainEvent);
    fn subscribe(&self) -> broadcast::Receiver<DomainEvent>;
    fn take_reliable_rx(&self) -> Option<mpsc::Receiver<DomainEvent>>;
}

/// Default single-node Tokio bus with reliable MPSC queue + broadcast fanout.
pub struct InMemoryEventBus {
    broadcast_tx: broadcast::Sender<DomainEvent>,
    reliable_tx: mpsc::Sender<DomainEvent>,
    reliable_rx: Mutex<Option<mpsc::Receiver<DomainEvent>>>,
}

impl InMemoryEventBus {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (broadcast_tx, _) = broadcast::channel(capacity);
        let (reliable_tx, reliable_rx) = mpsc::channel(capacity.max(256));
        Arc::new(Self {
            broadcast_tx,
            reliable_tx,
            reliable_rx: Mutex::new(Some(reliable_rx)),
        })
    }
}

impl EventBus for InMemoryEventBus {
    fn publish(&self, event: DomainEvent) {
        if let Err(err) = self.reliable_tx.try_send(event.clone()) {
            tracing::error!("reliable event queue saturated or closed: {err}");
        }
        if let Err(err) = self.broadcast_tx.send(event) {
            tracing::debug!("broadcast event published with zero subscribers: {err}");
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.broadcast_tx.subscribe()
    }

    fn take_reliable_rx(&self) -> Option<mpsc::Receiver<DomainEvent>> {
        self.reliable_rx.lock().ok().and_then(|mut g| g.take())
    }
}
