use std::net::Ipv4Addr;
use std::{error::Error, time::Duration};

use libp2p::futures::{FutureExt, StreamExt};
use libp2p::multihash::Multihash;
use libp2p::{
    core::{
        multiaddr::{Multiaddr, Protocol},
        transport::{OrTransport, Transport},
        upgrade,
    },
    dcutr, dns, identify, identity, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp, yamux, PeerId,
};

#[derive(NetworkBehaviour)]
struct Behaviour {
    relay_client: relay::client::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    dcutr: dcutr::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    log::info!("Local peer id: {}", local_peer_id);

    let (relay_transport, client) = relay::client::new(local_peer_id);

    let transport = OrTransport::new(
        relay_transport,
        dns::TokioDnsConfig::system(tcp::tokio::Transport::new(
            tcp::Config::default().port_reuse(true),
        ))?,
    )
    .upgrade(upgrade::Version::V1Lazy)
    .authenticate(
        noise::Config::new(&local_key).expect("Signing libp2p-noise static DH keypair failed."),
    )
    .multiplex(yamux::Config::default())
    .boxed();

    let behaviour = Behaviour {
        relay_client: client,
        ping: ping::Behaviour::new(ping::Config::new()),
        identify: identify::Behaviour::new(identify::Config::new(
            "/TODO/0.0.1".to_string(),
            local_key.public(),
        )),
        dcutr: dcutr::Behaviour::new(local_peer_id),
    };

    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

    swarm.listen_on(
        Multiaddr::empty()
            .with(Protocol::from(Ipv4Addr::UNSPECIFIED))
            .with(Protocol::Tcp(0)),
    )?;

    loop {
        tokio::select! {
            event = swarm.select_next_some().fuse() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        log::info!("Listening on {:?}", address);
                    }
                    event => panic!("{event:?}"),
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)).fuse() => {
                // Likely listening on all interfaces now, thus continuing by breaking the loop.
                break;
            }
        }
    }

    let relay_address =
        "/ip4/34.74.197.69/tcp/80/p2p/12D3KooWEk1iKyAYdcWUjs8uDUw1AuxCwKYir2qfF6sNW8RQ2XjU";

    // Connect to the relay server. Not for the reservation or relayed connection, but to (a) learn
    // our local public address and (b) enable a freshly started relay to learn its public address.
    swarm.dial(relay_address.parse::<Multiaddr>()?)?;

    let mut learned_observed_addr = false;
    let mut told_relay_observed_addr = false;

    log::info!("Waiting for events...");
    loop {
        match swarm.next().await.unwrap() {
            SwarmEvent::NewListenAddr { .. } => {}
            SwarmEvent::Dialing { .. } => {}
            SwarmEvent::ConnectionEstablished { .. } => {}
            SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {}
            SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Sent { .. })) => {
                log::info!("Told relay its public address.");
                told_relay_observed_addr = true;
            }
            SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                info: identify::Info { observed_addr, .. },
                ..
            })) => {
                log::info!("Relay told us our public address: {:?}", observed_addr);
                learned_observed_addr = true;
            }
            event => panic!("{event:?}"),
        }

        if learned_observed_addr && told_relay_observed_addr {
            break;
        }
    }

    let mode: u32 = std::env::args().nth(1).unwrap().parse()?;
    let remote_peer_id = std::env::args().nth(2).unwrap();

    if mode == 0 {
        swarm
            .dial(
                relay_address
                    .parse::<Multiaddr>()?
                    .with(Protocol::P2pCircuit)
                    .with(Protocol::P2p(Multihash::from_bytes(
                        remote_peer_id.as_bytes(),
                    )?)),
            )
            .unwrap();
    } else {
        swarm
            .listen_on(
                relay_address
                    .parse::<Multiaddr>()?
                    .with(Protocol::P2pCircuit),
            )
            .unwrap();
    }

    loop {
        match swarm.next().await.unwrap() {
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {:?}", address);
            }
            SwarmEvent::Behaviour(BehaviourEvent::RelayClient(
                relay::client::Event::ReservationReqAccepted { .. },
            )) => {
                assert!(mode == 1);
                log::info!("Relay accepted our reservation request.");
            }
            SwarmEvent::Behaviour(BehaviourEvent::RelayClient(event)) => {
                log::info!("{:?}", event)
            }
            SwarmEvent::Behaviour(BehaviourEvent::Dcutr(event)) => {
                log::info!("{:?}", event)
            }
            SwarmEvent::Behaviour(BehaviourEvent::Identify(event)) => {
                log::info!("{:?}", event)
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                log::info!("Established connection to {:?} via {:?}", peer_id, endpoint);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                log::info!("Outgoing connection error to {:?}: {:?}", peer_id, error);
            }
            _ => {}
        }
    }
}
