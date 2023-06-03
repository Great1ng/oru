use libp2p::swarm::NetworkBehaviour;
use libp2p::{dcutr, identify, relay, rendezvous};
use libp2p_identity::Keypair;

use super::NodeId;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct Behaviour {
    identify: identify::Behaviour,
    relay_client: relay::client::Behaviour,
    dcutr: dcutr::Behaviour,
    rendezvous: rendezvous::client::Behaviour,
}

impl Behaviour {
    pub fn new(
        local_keypair: Keypair,
        local_node_id: NodeId,
        relay_behaviour: relay::client::Behaviour,
    ) -> Self {
        Self {
            identify: identify::Behaviour::new(identify::Config::new(
                "/oru/0.1.0".to_owned(),
                local_keypair.public(),
            )),
            relay_client: relay_behaviour,
            dcutr: dcutr::Behaviour::new(local_node_id),
            rendezvous: rendezvous::client::Behaviour::new(local_keypair),
        }
    }

    pub fn discover(&mut self, rendezvous_node: NodeId) {
        self.rendezvous.discover(None, None, None, rendezvous_node)
    }

    pub fn register(&mut self, rendezvous_node: NodeId) {
        self.rendezvous.register(
            rendezvous::Namespace::from_static("oru"),
            rendezvous_node,
            None,
        )
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Event {
    Identify(identify::Event),
    Relay(relay::client::Event),
    Dcutr(dcutr::Event),
    Rendezvous(rendezvous::client::Event),
}

impl From<identify::Event> for Event {
    fn from(value: identify::Event) -> Self {
        Self::Identify(value)
    }
}

impl From<relay::client::Event> for Event {
    fn from(value: relay::client::Event) -> Self {
        Self::Relay(value)
    }
}

impl From<dcutr::Event> for Event {
    fn from(value: dcutr::Event) -> Self {
        Self::Dcutr(value)
    }
}

impl From<rendezvous::client::Event> for Event {
    fn from(value: rendezvous::client::Event) -> Self {
        Self::Rendezvous(value)
    }
}
