use tokio::sync::mpsc;

pub use syntrix_network::codecs::InvitePayload;

/// In-memory queue for pending invites received from the P2P network.
/// Replaces the old libp2p ProtocolHandler approach.
#[derive(Debug, Clone)]
pub struct InviteHandler {
    queue: std::sync::Arc<std::sync::Mutex<Vec<InvitePayload>>>,
    event_tx: mpsc::UnboundedSender<InvitePayload>,
}

impl InviteHandler {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<InvitePayload>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self {
            queue: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            event_tx: tx,
        }, rx)
    }

    pub fn push(&self, payload: InvitePayload) {
        let _ = self.event_tx.send(payload.clone());
        self.queue.lock().unwrap().push(payload);
    }

    pub fn get_pending(&self) -> Vec<InvitePayload> {
        self.queue.lock().unwrap().drain(..).collect()
    }
}
