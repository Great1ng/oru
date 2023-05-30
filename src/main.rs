use std::error::Error;
use std::net::Ipv4Addr;

use libp2p::core::upgrade;
use libp2p::futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent};
use libp2p::{identify, noise, relay, tcp};
use libp2p::{Multiaddr, PeerId, Transport};

#[derive(NetworkBehaviour)]
struct Behaviour {
    relay: relay::Behaviour,
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut args = std::env::args();
    let port: u16 = args.nth(1).expect("Provide port.").parse()?;
    log::info!("Listening on port {}", port);

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    log::info!("Local peer id: {}", local_peer_id);

    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default());

    let transport = tcp_transport
        .upgrade(upgrade::Version::V1Lazy)
        .authenticate(
            noise::Config::new(&local_key).expect("Signing libp2p-noise static DH keypair failed."),
        )
        .multiplex(libp2p::yamux::Config::default())
        .boxed();

    let behaviour = Behaviour {
        relay: relay::Behaviour::new(local_peer_id, Default::default()),
        identify: identify::Behaviour::new(identify::Config::new(
            "/TODO/0.0.1".to_owned(),
            local_key.public(),
        )),
    };

    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

    let listen_addr = Multiaddr::empty()
        .with(Protocol::from(Ipv4Addr::UNSPECIFIED))
        .with(Protocol::Tcp(port));

    swarm.listen_on(listen_addr)?;

    loop {
        match swarm.next().await.expect("Infinite stream") {
            SwarmEvent::Behaviour(event) => {
                log::info!("{event:?}")
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {address:?}");
            }
            _ => {}
        }
    }
}
