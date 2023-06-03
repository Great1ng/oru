use std::error::Error;

use libp2p::multiaddr::Protocol;
use libp2p::rendezvous;
use libp2p::{
    futures::StreamExt,
    swarm::{derive_prelude::ListenerId, SwarmEvent},
    Multiaddr,
};

use super::{Event, Node, NodeId};

pub struct Connection<'a> {
    node: &'a mut Node,
    public_listener: Option<ListenerId>,
    rendezvous_node_id: NodeId,
    rendezvous_node_addr: Multiaddr,
}

impl<'a> Connection<'a> {
    pub fn new(
        node: &'a mut Node,
        rendezvous_node_id: NodeId,
        rendezvous_node_addr: Multiaddr,
    ) -> Self {
        Self {
            node,
            public_listener: None,
            rendezvous_node_id,
            rendezvous_node_addr,
        }
    }

    pub async fn handle(&mut self) -> Result<(), Box<dyn Error>> {
        self.node
            .swarm
            .behaviour_mut()
            .discover(self.rendezvous_node_id);

        loop {
            match self.node.swarm.next().await {
                Some(SwarmEvent::Behaviour(Event::Rendezvous(event))) => match event {
                    rendezvous::client::Event::Discovered {
                        rendezvous_node,
                        registrations,
                        ..
                    } => {
                        if registrations.is_empty() {
                            self.public_listener = Some(self.node.swarm.listen_on(
                                self.rendezvous_node_addr.clone().with(Protocol::P2pCircuit),
                            )?);

                            if !self.node.wait_for_reservation().await {
                                //TODO: retry or error instead of panic
                                panic!("Reservation failed");
                            }

                            self.node.swarm.behaviour_mut().register(rendezvous_node);
                        } else {
                            for registration in registrations {
                                let dial_addr = self
                                    .rendezvous_node_addr
                                    .clone()
                                    .with(Protocol::P2pCircuit)
                                    .with(Protocol::P2p(registration.record.peer_id().into()));

                                self.node.swarm.dial(dial_addr)?;
                            }
                        }
                    }
                    rendezvous::client::Event::RegisterFailed(error) => {
                        panic!("Registration failed due to {error}");
                    }
                    _ => {}
                },
                Some(event) => println!("{:#?}", event),
                None => break,
            }
        }

        Ok(())
    }
}
