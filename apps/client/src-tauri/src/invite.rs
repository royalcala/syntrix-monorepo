use std::sync::{Arc, Mutex};
use iroh::{endpoint::Connection, protocol::ProtocolHandler};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// ALPN for invite protocol: admin sends org tickets to client devices.
pub const INVITE_ALPN: &[u8] = b"/syntrix/invite/1";

/// Payload received from admin when a device is invited to an org.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InvitePayload {
    pub org_name: String,
    pub role: String,
    /// The admin's device address (for bootstrapping P2P sync after joining).
    /// Format: semicolon-separated or JSON object (see parse_device_addr).
    pub admin_addr: Option<String>,
    pub tickets: Vec<TicketInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TicketInfo {
    pub ns: String,
    pub ticket: String,
}

/// Stores received invites + emits Tauri events.
#[derive(Debug, Clone)]
pub struct InviteProtocolHandler {
    pub queue: Arc<Mutex<Vec<InvitePayload>>>,
    /// Channel sender for real-time event emission to the frontend.
    event_tx: mpsc::UnboundedSender<InvitePayload>,
}

impl InviteProtocolHandler {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<InvitePayload>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            event_tx: tx,
        }, rx)
    }

    pub fn get_pending(&self) -> Vec<InvitePayload> {
        self.queue.lock().unwrap().drain(..).collect()
    }
}

impl ProtocolHandler for InviteProtocolHandler {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl std::future::Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        let queue = self.queue.clone();
        let tx = self.event_tx.clone();
        async move {
            let mut recv = connection.accept_uni().await.map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("accept_uni: {e}"))
                ))
            })?;

            let buf = recv.read_to_end(65536).await.map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("read_to_end: {e}"))
                ))
            })?;

            match serde_json::from_slice::<InvitePayload>(&buf) {
                Ok(invite) => {
                    eprintln!("invite: received invite for org {} as {}", invite.org_name, invite.role);
                    // Emit event to frontend immediately (non-blocking)
                    let _ = tx.send(invite.clone());
                    // Also store in queue as fallback
                    queue.lock().unwrap().push(invite);
                }
                Err(e) => {
                    eprintln!("invite: failed to parse payload: {e}");
                }
            }

            Ok(())
        }
    }
}
