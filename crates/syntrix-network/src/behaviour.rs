use libp2p::{
    gossipsub, identify, kad, ping, request_response,
};
use libp2p_swarm_derive::NetworkBehaviour;

use crate::codecs::{CombinedCodec, NetworkRequest, NetworkResponse};

pub enum CustomBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    Ping(ping::Event),
    Rr(request_response::Event<NetworkRequest, NetworkResponse>),
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

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CustomBehaviourEvent")]
pub struct CustomBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub rr: request_response::Behaviour<CombinedCodec>,
}
