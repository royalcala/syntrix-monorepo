use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, mdns, ping, relay, request_response,
};
use libp2p_swarm_derive::NetworkBehaviour;

use crate::codecs::{CombinedCodec, NetworkRequest, NetworkResponse};

pub enum CustomBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    Ping(ping::Event),
    Rr(request_response::Event<NetworkRequest, NetworkResponse>),
    Autonat(autonat::Event),
    RelayClient(relay::client::Event),
    Dcutr(dcutr::Event),
    Mdns(mdns::Event),
}

impl From<gossipsub::Event> for CustomBehaviourEvent {
    fn from(e: gossipsub::Event) -> Self { Self::Gossipsub(e) }
}
impl From<kad::Event> for CustomBehaviourEvent {
    fn from(e: kad::Event) -> Self { Self::Kademlia(e) }
}
impl From<identify::Event> for CustomBehaviourEvent {
    fn from(e: identify::Event) -> Self { Self::Identify(e) }
}
impl From<ping::Event> for CustomBehaviourEvent {
    fn from(e: ping::Event) -> Self { Self::Ping(e) }
}
impl From<request_response::Event<NetworkRequest, NetworkResponse>> for CustomBehaviourEvent {
    fn from(e: request_response::Event<NetworkRequest, NetworkResponse>) -> Self { Self::Rr(e) }
}
impl From<autonat::Event> for CustomBehaviourEvent {
    fn from(e: autonat::Event) -> Self { Self::Autonat(e) }
}
impl From<relay::client::Event> for CustomBehaviourEvent {
    fn from(e: relay::client::Event) -> Self { Self::RelayClient(e) }
}
impl From<dcutr::Event> for CustomBehaviourEvent {
    fn from(e: dcutr::Event) -> Self { Self::Dcutr(e) }
}
impl From<mdns::Event> for CustomBehaviourEvent {
    fn from(e: mdns::Event) -> Self { Self::Mdns(e) }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CustomBehaviourEvent")]
pub struct CustomBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub rr: request_response::Behaviour<CombinedCodec>,
    pub autonat: autonat::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}
