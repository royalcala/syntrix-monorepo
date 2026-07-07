mod behaviour;
pub mod cdc;
pub mod codecs;
pub mod schema;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::StreamExt;
use libp2p::gossipsub::{IdentTopic, MessageAuthenticity};
use libp2p::identity::Keypair;
use libp2p::kad::{store::MemoryStore, Mode, RecordKey};
use libp2p::request_response::{OutboundRequestId, ProtocolSupport, ResponseChannel};
use libp2p::swarm::SwarmEvent;
use libp2p::{autonat, dcutr, noise, relay, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::sync::{mpsc, oneshot, RwLock};

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
    BlockPeer(PeerId),
    EnsureConnected { peer_id: PeerId, addrs: Vec<Multiaddr> },
    DiscoverOrgPeers { org_id: String, result_tx: oneshot::Sender<Vec<PeerId>> },
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
    listen_addrs: Arc<RwLock<Vec<Multiaddr>>>,
    blocked_peers: Arc<RwLock<HashSet<PeerId>>>,
}

#[derive(Clone)]
struct PeerScore {
    connected_since: Option<Instant>,
    last_disconnect: Option<Instant>,
    disconnect_count: u32,
    avg_latency_ms: f64,
}

fn backoff_delay(attempt: u32) -> Duration {
    let secs = 2u64.saturating_pow(attempt.min(6));
    Duration::from_secs(secs)
}

impl P2PNode {
    pub async fn new(config: NetworkConfig) -> Result<(Self, EventReceiver)> {
        let local_peer_id = config.keypair.public().to_peer_id();
        let peer_bytes = peer_id_to_bytes(local_peer_id);

        let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(5))
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

        let autonat = autonat::Behaviour::new(local_peer_id, autonat::Config::default());

        let dcutr = dcutr::Behaviour::new(local_peer_id);

        let (_relay_transport, relay_client) = relay::client::new(local_peer_id);

        let behaviour = CustomBehaviour {
            gossipsub,
            kademlia,
            identify,
            ping,
            rr,
            autonat,
            relay_client,
            dcutr,
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

        // Bootstrap Kademlia with provided bootstrap nodes
        let local_id = *swarm.local_peer_id();
        for addr in &config.bootstrap_nodes {
            let _ = swarm.add_peer_address(local_id, addr.clone());
        }
        let _ = swarm.behaviour_mut().kademlia.bootstrap();

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let listen_addrs = Arc::new(RwLock::new(Vec::new()));
        let blocked_peers = Arc::new(RwLock::new(HashSet::new()));

        let addr_clone = listen_addrs.clone();
        let blocked_clone = blocked_peers.clone();
        tokio::spawn(run_event_loop(swarm, cmd_rx, cmd_tx.clone(), event_tx, addr_clone, blocked_clone));

        Ok((
            Self {
                cmd_tx,
                local_peer_id,
                local_peer_bytes: peer_bytes,
                listen_addrs,
                blocked_peers,
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

    pub fn block_peer(&self, peer_id: PeerId) {
        let mut blocked = self.blocked_peers.blocking_write();
        blocked.insert(peer_id);
        let _ = self.cmd_tx.send(Command::BlockPeer(peer_id));
    }

    pub fn ensure_connected(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
        let _ = self.cmd_tx.send(Command::EnsureConnected { peer_id, addrs });
    }

    pub async fn discover_org_peers(&self, org_id: &str) -> Vec<PeerId> {
        let hash = blake3::hash(format!("syntrix-p2p-{}", org_id).as_bytes());
        let key = RecordKey::new(hash.as_bytes());
        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send(Command::DiscoverOrgPeers { org_id: org_id.to_string(), result_tx: tx }).is_err() {
            return vec![];
        }
        rx.await.unwrap_or_default()
    }

    pub fn local_peer_id(&self) -> PeerId { self.local_peer_id }
    pub fn local_peer_id_bytes(&self) -> [u8; 32] { self.local_peer_bytes }
    pub async fn listen_addrs(&self) -> Vec<Multiaddr> { self.listen_addrs.read().await.clone() }
}

fn peer_id_to_bytes(peer_id: PeerId) -> [u8; 32] {
    let bytes = peer_id.to_bytes();
    let mut arr = [0u8; 32];
    let start = bytes.len().saturating_sub(32);
    let len = bytes.len().saturating_sub(start);
    arr[..len].copy_from_slice(&bytes[start..start + len]);
    arr
}

async fn run_event_loop(
    mut swarm: Swarm<CustomBehaviour>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    cmd_tx: mpsc::UnboundedSender<Command>,
    event_tx: mpsc::UnboundedSender<Event>,
    listen_addrs: Arc<RwLock<Vec<Multiaddr>>>,
    blocked_peers: Arc<RwLock<HashSet<PeerId>>>,
) {
    let mut pending_catchup: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<serde_json::Value>>>> = HashMap::new();
    let mut incoming_catchup: HashMap<u64, ResponseChannel<NetworkResponse>> = HashMap::new();
    let mut next_response_id: u64 = 0;
    let mut _pending_invites: HashMap<OutboundRequestId, ()> = HashMap::new();
    let mut peer_scores: HashMap<PeerId, PeerScore> = HashMap::new();
    let mut reconnect_tasks: HashMap<PeerId, tokio::sync::oneshot::Sender<()>> = HashMap::new();
    let mut active_kademlia_key: Option<RecordKey> = None;
    let mut last_reannounce: Option<Instant> = None;

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
                Command::BlockPeer(peer_id) => {
                    let _ = swarm.disconnect_peer_id(peer_id);
                    reconnect_tasks.remove(&peer_id);
                }
                Command::EnsureConnected { peer_id, addrs } => {
                    if reconnect_tasks.contains_key(&peer_id) {
                        continue;
                    }
                    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
                    reconnect_tasks.insert(peer_id, cancel_tx);
                    let event_tx_clone = event_tx.clone();
                    let blocked = blocked_peers.clone();
                    let score = peer_scores.get(&peer_id).cloned();
                    let cmd_tx = cmd_tx.clone();
                    tokio::spawn(async move {
                        reconnect_loop(peer_id, addrs, score, cancel_rx, cmd_tx, event_tx_clone, blocked).await;
                    });
                }
                Command::DiscoverOrgPeers { org_id, result_tx } => {
                    let hash = blake3::hash(format!("syntrix-p2p-{}", org_id).as_bytes());
                    let key = RecordKey::new(hash.as_bytes());
                    swarm.behaviour_mut().kademlia.start_providing(key.clone()).ok();
                    active_kademlia_key = Some(key.clone());
                    swarm.behaviour_mut().kademlia.get_providers(key);
                    let _ = result_tx.send(vec![]);
                }
                Command::Shutdown => break,
            },
            Some(ev) = swarm.next() => {
                handle_swarm_event(
                    &mut swarm, ev, &event_tx,
                    &mut pending_catchup, &mut _pending_invites,
                    &mut incoming_catchup, &mut next_response_id,
                    &mut peer_scores, &reconnect_tasks,
                    &blocked_peers, &active_kademlia_key,
                    &mut last_reannounce,
                );
                let addrs: Vec<Multiaddr> = swarm.listeners().cloned().collect();
                *listen_addrs.write().await = addrs;
            }
            else => break,
        }
    }
}

async fn reconnect_loop(
    peer_id: PeerId,
    _addrs: Vec<Multiaddr>,
    score: Option<PeerScore>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
    cmd_tx: mpsc::UnboundedSender<Command>,
    event_tx: mpsc::UnboundedSender<Event>,
    blocked_peers: Arc<RwLock<HashSet<PeerId>>>,
) {
    let initial_delay = match &score {
        Some(s) if s.disconnect_count < 3 => Duration::from_secs(1),
        Some(s) if s.disconnect_count < 10 => Duration::from_secs(2),
        Some(_) => Duration::from_secs(4),
        None => Duration::from_secs(2),
    };
    let delays = [
        initial_delay,
        Duration::from_secs(4),
        Duration::from_secs(8),
        Duration::from_secs(16),
        Duration::from_secs(32),
        Duration::from_secs(64),
    ];
    let mut cancel_rx = cancel_rx;
    for delay in delays {
        if blocked_peers.read().await.contains(&peer_id) { return; }
        tokio::select! {
            _ = &mut cancel_rx => return,
            _ = tokio::time::sleep(delay) => {
                let _ = cmd_tx.send(Command::Dial(Multiaddr::empty()));
            }
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
    peer_scores: &mut HashMap<PeerId, PeerScore>,
    _reconnect_tasks: &HashMap<PeerId, tokio::sync::oneshot::Sender<()>>,
    blocked_peers: &Arc<RwLock<HashSet<PeerId>>>,
    _active_kademlia_key: &Option<RecordKey>,
    _last_reannounce: &mut Option<Instant>,
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
                tracing::warn!(
                    target: "syntrix",
                    error = ?error,
                    "invite delivery failed (OutboundFailure). Did you dial the peer first?",
                );
                pending_invites.remove(&request_id);
            }
        }
        SwarmEvent::Behaviour(CustomBehaviourEvent::Identify(_)) => {}
        SwarmEvent::Behaviour(CustomBehaviourEvent::Ping(_)) => {}
        SwarmEvent::Behaviour(CustomBehaviourEvent::Kademlia(_)) => {}
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            if blocked_peers.blocking_read().contains(&peer_id) {
                let _ = swarm.disconnect_peer_id(peer_id);
                return;
            }
            tracing::info!(target: "syntrix", peer = %peer_id, "P2P connection established");
            let _ = event_tx.send(Event::PeerConnected(peer_id));
            peer_scores.entry(peer_id).or_insert(PeerScore {
                connected_since: Some(Instant::now()),
                last_disconnect: None,
                disconnect_count: 0,
                avg_latency_ms: 0.0,
            });
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            tracing::info!(target: "syntrix", peer = %peer_id, "P2P connection closed");
            let _ = event_tx.send(Event::PeerDisconnected(peer_id));
            let score = peer_scores.entry(peer_id).or_insert(PeerScore {
                connected_since: None,
                last_disconnect: None,
                disconnect_count: 0,
                avg_latency_ms: 0.0,
            });
            score.connected_since = None;
            score.last_disconnect = Some(Instant::now());
            score.disconnect_count = score.disconnect_count.saturating_add(1);
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!(target: "syntrix", addr = %address, "new listen address");
            let should_reannounce = _last_reannounce.map(|t| t.elapsed() > Duration::from_secs(5)).unwrap_or(true);
            if should_reannounce {
                if let Some(key) = _active_kademlia_key.clone() {
                    let _ = swarm.behaviour_mut().kademlia.start_providing(key);
                }
                let _ = swarm.behaviour_mut().kademlia.bootstrap();
                *_last_reannounce = Some(Instant::now());
            }
        }
        _ => {}
    }
}
