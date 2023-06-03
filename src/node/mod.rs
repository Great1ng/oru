use std::error::Error;
use std::net::Ipv4Addr;

use libp2p::core::transport::upgrade;
use libp2p::futures::{FutureExt, StreamExt};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
// use libp2p::futures::StreamExt;
use libp2p::{identify, noise, relay, swarm, tcp, yamux, Multiaddr};
use libp2p::{PeerId, Swarm, Transport};
use libp2p_identity::{Keypair, PublicKey};

mod behaviour;
pub use behaviour::*;

mod connection;
pub use connection::*;

pub type NodeId = PeerId;

pub struct Node {
    keypair: Keypair,
    swarm: Swarm<Behaviour>,
}

impl Node {
    pub fn new() -> Self {
        let local_keypair = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from_public_key(&local_keypair.public());

        let transport_config = tcp::Config::new().port_reuse(true);
        let noise = noise::Config::new(&local_keypair).expect("Problem signing with DH keys");
        let (relay_transport, relay_behaviour) = relay::client::new(local_peer_id);
        let transport = tcp::tokio::Transport::new(transport_config)
            .or_transport(relay_transport)
            .upgrade(upgrade::Version::V1)
            .authenticate(noise)
            .multiplex(yamux::Config::default())
            .boxed();

        let behaviour = Behaviour::new(local_keypair.clone(), local_peer_id, relay_behaviour);

        let swarm =
            swarm::SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

        Self {
            keypair: local_keypair,
            swarm,
        }
    }

    pub fn get_id(&self) -> &NodeId {
        self.swarm.local_peer_id()
    }

    #[allow(dead_code)]
    pub fn public_key(&self) -> PublicKey {
        self.keypair.public()
    }

    pub async fn connect(
        &mut self,
        boot_node_addr: &str,
        preferred_port: Option<u16>,
    ) -> Result<Connection, Box<dyn Error>> {
        let mut boot_node_addr = boot_node_addr.parse::<Multiaddr>()?;

        let local_addr = Multiaddr::empty()
            .with(Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
            .with(Protocol::Tcp(preferred_port.unwrap_or(0)));

        let _local_listener = self.swarm.listen_on(local_addr)?;

        self.swarm.dial(boot_node_addr.clone())?;

        let events = self
            .wait_sent_receive()
            .await
            .expect("Stream finished before events happened");

        let mut _local_public_addr = Multiaddr::empty();
        let mut boot_node_id = None;

        for event in events {
            match event {
                identify::Event::Sent { peer_id } => {
                    boot_node_id = Some(peer_id);
                    boot_node_addr.push(Protocol::P2p(peer_id.into()));
                }
                identify::Event::Received { info, .. } => {
                    _local_public_addr = info.observed_addr;
                }
                _ => panic!("Unexpected event: {:#?}", event),
            }
        }

        Ok(Connection::new(
            self,
            boot_node_id.expect("Must have value"),
            boot_node_addr,
        ))
    }

    async fn wait_for_reservation(&mut self) -> bool {
        loop {
            let event = self.swarm.next().fuse().await;

            match event {
                Some(SwarmEvent::Behaviour(Event::Relay(event))) => match event {
                    relay::client::Event::ReservationReqAccepted { .. } => break true,
                    relay::client::Event::ReservationReqFailed { error, .. } => {
                        println!("{error}");
                        break false;
                    }
                    _ => {}
                },
                Some(_) => {}
                None => break false,
            }
        }
    }

    //TODO: proc macro
    //TODO: tuple instead of vec
    async fn wait_sent_receive(&mut self) -> Option<Vec<identify::Event>> {
        let mut events = [None, None];
        let mut events_count = 0;

        loop {
            let event = self.swarm.next().fuse().await;

            match event {
                Some(SwarmEvent::Behaviour(Event::Identify(event))) => match event {
                    identify::Event::Sent { .. } => {
                        events_count += events[0].is_none() as usize;
                        events[0] = Some(event);
                    }
                    identify::Event::Received { .. } => {
                        events_count += events[1].is_none() as usize;
                        events[1] = Some(event);
                    }
                    _ => {}
                },
                Some(_) => {}
                None => return None,
            }

            if events_count == 2 {
                break;
            }
        }

        Some(
            events
                .into_iter()
                .map(|maybe_event| maybe_event.unwrap())
                .collect(),
        )
    }
}
