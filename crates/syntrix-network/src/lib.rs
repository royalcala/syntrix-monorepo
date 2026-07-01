mod behaviour;
pub mod cdc;
pub mod codecs;

use std::collections::HashMap;

use anyhow::{Context, Result};
use futures::StreamExt;
use libp2p::gossipsub::{IdentTopic, MessageAuthenticity};
use libp2p::identity::Keypair;
use libp2p::kad::{store::MemoryStore, Mode};
use libp2p::request_response::{OutboundRequestId, ProtocolSupport, ResponseChannel};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub use behaviour::{CustomBehaviour, CustomBehaviourEvent};
pub use codecs::{InvitePayload, NetworkRequest, NetworkResponse};

pub type EventReceiver = mpsc::UnboundedReceiver<Event>;

#[derive(Debug)]
pub enum Event {
    GossipsubMessage { source: PeerId, topic: String, data: Vec<u8> },
    InviteReceived { peer: PeerId, payload: InvitePayload },
    CatchupRequestReceived { peer: PeerId, org_id: String, since_hlc: u64, response_id: u64 },
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

enum Command {
    JoinTopic(String),
    Publish { topic: String, data: Vec<u8> },
    SendInvite { peer: PeerId, payload: InvitePayload },
    RequestCatchup {
        peer: PeerId,
        org_id: String,
        since_hlc: u64,
        response_tx: oneshot::Sender<Result<Vec<serde_json::Value>>>,
    },
    Dial(Multiaddr),
    RespondCatchup { response_id: u64, result: Result<Vec<serde_json::Value>> },
    #[allow(dead_code)]
    Shutdown,
}

pub struct NetworkConfig {
    pub keypair: Keypair,
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub data_dir: std::path::PathBuf,
}

#[derive(Clone)]
pub struct P2PNode {
    cmd_tx: mpsc::UnboundedSender<Command>,
    local_peer_id: PeerId,
    local_peer_bytes: [u8; 32],
    listen_addrs: std::sync::Arc<tokio::sync::RwLock<Vec<Multiaddr>>>,
}

impl P2PNode {
    pub async fn new(config: NetworkConfig) -> Result<(Self, EventReceiver)> {
        let local_peer_id = config.keypair.public().to_peer_id();
        let peer_bytes = peer_id_to_bytes(local_peer_id);

        let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
            .heartbeat_interval(std::time::Duration::from_secs(5))
            .validation_mode(libp2p::gossipsub::ValidationMode::Permissive)
            .build()
            .context("gossipsub config")?;

        let gossipsub = libp2p::gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(config.keypair.clone()),
            gossipsub_config,
        )
        .map_err(|e| anyhow::anyhow!("gossipsub: {e}"))?;

        let kademlia_store = MemoryStore::new(local_peer_id);
        let mut kademlia = libp2p::kad::Behaviour::new(local_peer_id, kademlia_store);
        kademlia.set_mode(Some(Mode::Server));

        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new("/syntrix/1.0.0".to_string(), config.keypair.public()),
        );
        let ping = libp2p::ping::Behaviour::new(libp2p::ping::Config::default());

        let rr = libp2p::request_response::Behaviour::new(
            [
                ("/syntrix/invite/1".to_string(), ProtocolSupport::Full),
                ("/syntrix/catchup/1".to_string(), ProtocolSupport::Full),
            ],
            libp2p::request_response::Config::default(),
        );

        let behaviour = CustomBehaviour {
            gossipsub,
            kademlia,
            identify,
            ping,
            rr,
        };

        let mut swarm = SwarmBuilder::with_existing_identity(config.keypair)
            .with_tokio()
            .with_quic()
            .with_dns()?
            .with_behaviour(|_| behaviour)?
            .build();

        for addr in &config.listen_on {
            swarm.listen_on(addr.clone())?;
        }

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let listen_addrs = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));

        let addr_clone = listen_addrs.clone();
        tokio::spawn(run_event_loop(swarm, cmd_rx, event_tx, addr_clone));

        Ok((
            Self {
                cmd_tx,
                local_peer_id,
                local_peer_bytes: peer_bytes,
                listen_addrs,
            },
            event_rx,
        ))
    }

    pub fn join_topic(&self, topic: &str) -> Result<()> {
        self.cmd_tx.send(Command::JoinTopic(topic.to_string()))
            .map_err(|_| anyhow::anyhow!("event loop closed"))
    }

    pub fn publish(&self, topic: &str, data: Vec<u8>) -> Result<()> {
        self.cmd_tx.send(Command::Publish { topic: topic.to_string(), data })
            .map_err(|_| anyhow::anyhow!("event loop closed"))
    }

    pub fn send_invite(&self, peer: PeerId, payload: InvitePayload) -> Result<()> {
        self.cmd_tx.send(Command::SendInvite { peer, payload })
            .map_err(|_| anyhow::anyhow!("event loop closed"))
    }

    pub async fn request_catchup(&self, peer: PeerId, org_id: String, since_hlc: u64) -> Result<Vec<serde_json::Value>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::RequestCatchup { peer, org_id, since_hlc, response_tx: tx })
            .map_err(|_| anyhow::anyhow!("event loop closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("event loop dropped"))?
    }

    pub fn respond_catchup(&self, response_id: u64, events: Vec<serde_json::Value>) -> Result<()> {
        self.cmd_tx.send(Command::RespondCatchup { response_id, result: Ok(events) })
            .map_err(|_| anyhow::anyhow!("event loop closed"))
    }

    pub fn dial(&self, addr: Multiaddr) -> Result<()> {
        self.cmd_tx.send(Command::Dial(addr))
            .map_err(|_| anyhow::anyhow!("event loop closed"))
    }

    pub fn local_peer_id(&self) -> PeerId { self.local_peer_id }
    pub fn local_peer_id_bytes(&self) -> [u8; 32] { self.local_peer_bytes }
    pub async fn listen_addrs(&self) -> Vec<Multiaddr> { self.listen_addrs.read().await.clone() }
}

fn peer_id_to_bytes(peer_id: PeerId) -> [u8; 32] {
    let bytes = peer_id.to_bytes();
    let mut arr = [0u8; 32];
    let len = bytes.len().min(32);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

async fn run_event_loop(
    mut swarm: Swarm<CustomBehaviour>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    event_tx: mpsc::UnboundedSender<Event>,
    listen_addrs: std::sync::Arc<tokio::sync::RwLock<Vec<Multiaddr>>>,
) {
    let mut pending_catchup: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<serde_json::Value>>>> = HashMap::new();
    let mut incoming_catchup: HashMap<u64, ResponseChannel<NetworkResponse>> = HashMap::new();
    let mut next_response_id: u64 = 0;
    let mut _pending_invites: HashMap<OutboundRequestId, ()> = HashMap::new();

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => match cmd {
                Command::JoinTopic(topic_str) => {
                    let topic = IdentTopic::new(topic_str.clone());
                    if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                        tracing::warn!("subscribe to {topic_str}: {e}");
                    }
                }
                Command::Publish { topic, data } => {
                    let topic = IdentTopic::new(topic);
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
                        tracing::warn!("publish: {e}");
                    }
                }
                Command::SendInvite { peer, payload } => {
                    let req = NetworkRequest::Invite(payload);
                    let id = swarm.behaviour_mut().rr.send_request(&peer, req);
                    _pending_invites.insert(id, ());
                }
                Command::RequestCatchup { peer, org_id, since_hlc, response_tx } => {
                    let req = NetworkRequest::CatchupRequest { org_id, since_hlc };
                    let id = swarm.behaviour_mut().rr.send_request(&peer, req);
                    pending_catchup.insert(id, response_tx);
                }
                Command::RespondCatchup { response_id, result } => {
                    if let Some(channel) = incoming_catchup.remove(&response_id) {
                        let response = match result {
                            Ok(events) => NetworkResponse::CatchupResponse(events),
                            Err(_) => NetworkResponse::CatchupResponse(vec![]),
                        };
                        let _ = swarm.behaviour_mut().rr.send_response(channel, response);
                    }
                }
                Command::Dial(addr) => { if let Err(e) = swarm.dial(addr) { tracing::warn!("dial: {e}"); } }
                Command::Shutdown => break,
            },
            Some(ev) = swarm.next() => {
                handle_swarm_event(&mut swarm, ev, &event_tx, &mut pending_catchup, &mut _pending_invites, &mut incoming_catchup, &mut next_response_id);
                let addrs: Vec<Multiaddr> = swarm.listeners().cloned().collect();
                *listen_addrs.write().await = addrs;
            }
            else => break,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_swarm_event(
    swarm: &mut Swarm<CustomBehaviour>,
    event: SwarmEvent<CustomBehaviourEvent>,
    event_tx: &mpsc::UnboundedSender<Event>,
    pending_catchup: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<serde_json::Value>>>>,
    pending_invites: &mut HashMap<OutboundRequestId, ()>,
    incoming_catchup: &mut HashMap<u64, ResponseChannel<NetworkResponse>>,
    next_response_id: &mut u64,
) {
    match event {
        SwarmEvent::Behaviour(CustomBehaviourEvent::Gossipsub(
            libp2p::gossipsub::Event::Message { propagation_source, message_id: _, message },
        )) => {
            let topic = message.topic.to_string();
            let _ = event_tx.send(Event::GossipsubMessage {
                source: propagation_source, topic, data: message.data,
            });
        }
        SwarmEvent::Behaviour(CustomBehaviourEvent::Rr(
            libp2p::request_response::Event::Message {
                peer,
                message: libp2p::request_response::Message::Request {
                    request_id: _,
                    request,
                    channel,
                },
            },
        )) => {
            match request {
                NetworkRequest::Invite(payload) => {
                    let _ = event_tx.send(Event::InviteReceived { peer, payload });
                    let _ = swarm.behaviour_mut().rr.send_response(channel, NetworkResponse::InviteAck);
                }
                NetworkRequest::CatchupRequest { org_id, since_hlc } => {
                    let rid = *next_response_id;
                    *next_response_id += 1;
                    incoming_catchup.insert(rid, channel);
                    let _ = event_tx.send(Event::CatchupRequestReceived {
                        peer, org_id, since_hlc, response_id: rid,
                    });
                }
            }
        }
        SwarmEvent::Behaviour(CustomBehaviourEvent::Rr(
            libp2p::request_response::Event::Message {
                peer: _,
                message: libp2p::request_response::Message::Response { request_id, response },
            },
        )) => {
            if let Some(sender) = pending_catchup.remove(&request_id) {
                match response {
                    NetworkResponse::CatchupResponse(events) => { let _ = sender.send(Ok(events)); }
                    _ => { let _ = sender.send(Err(anyhow::anyhow!("unexpected response type"))); }
                }
            } else {
                pending_invites.remove(&request_id);
            }
        }
        SwarmEvent::Behaviour(CustomBehaviourEvent::Rr(
            libp2p::request_response::Event::OutboundFailure { request_id, error, .. },
        )) => {
            if let Some(sender) = pending_catchup.remove(&request_id) {
                let _ = sender.send(Err(anyhow::anyhow!("catchup request failed: {error:?}")));
            } else {
                pending_invites.remove(&request_id);
            }
        }
        SwarmEvent::Behaviour(CustomBehaviourEvent::Identify(_)) => {}
        SwarmEvent::Behaviour(CustomBehaviourEvent::Ping(_)) => {}
        SwarmEvent::Behaviour(CustomBehaviourEvent::Kademlia(_)) => {}
        SwarmEvent::ConnectionEstablished { peer_id, .. } => { let _ = event_tx.send(Event::PeerConnected(peer_id)); }
        SwarmEvent::ConnectionClosed { peer_id, .. } => { let _ = event_tx.send(Event::PeerDisconnected(peer_id)); }
        _ => {}
    }
}
