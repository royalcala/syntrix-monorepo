use std::time::Duration;
use clap::Parser;
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{identity, relay, SwarmBuilder};

#[derive(Parser)]
#[command(name = "syntrix-relay", about = "libp2p relay server for NAT traversal")]
struct Args {
    #[arg(long, default_value = "/ip4/0.0.0.0/udp/0/quic-v1")]
    listen: String,

    #[arg(long, default_value = "false")]
    tcp: bool,

    #[arg(long)]
    keypair_file: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syntrix_relay=info,libp2p_relay=info".into()),
        )
        .init();

    let args = Args::parse();

    let keypair = if let Some(ref path) = args.keypair_file {
        if let Ok(bytes) = std::fs::read(path) {
            identity::Keypair::from_protobuf_encoding(&bytes)
                .unwrap_or_else(|_| identity::Keypair::generate_ed25519())
        } else {
            let kp = identity::Keypair::generate_ed25519();
            if let Ok(encoded) = kp.to_protobuf_encoding() {
                let _ = std::fs::write(path, encoded);
            }
            kp
        }
    } else {
        identity::Keypair::generate_ed25519()
    };
    let peer_id = keypair.public().to_peer_id();

    tracing::info!("Relay server starting");
    tracing::info!("  Peer ID: {peer_id}");
    tracing::info!("  Listen:  {}", args.listen);

    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_dns()?
        .with_behaviour(|key| relay::Behaviour::new(
            key.public().to_peer_id(),
            relay::Config {
                max_reservations: 256,
                max_reservations_per_peer: 8,
                reservation_duration: Duration::from_secs(7200),
                reservation_rate_limiters: vec![],
                max_circuits: 512,
                max_circuits_per_peer: 16,
                max_circuit_duration: Duration::from_secs(3600),
                max_circuit_bytes: 1_073_741_824,
                circuit_src_rate_limiters: vec![],
            },
        ))?
        .build();

    swarm.listen_on(args.listen.parse()?)?;

    if args.tcp {
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    }

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                tracing::info!("Listening on {address}");
            }
            SwarmEvent::Behaviour(relay::Event::ReservationReqAccepted { .. }) => {
                tracing::debug!("relay reservation accepted");
            }
            SwarmEvent::Behaviour(relay::Event::CircuitReqAccepted { .. }) => {
                tracing::debug!("relay circuit request received");
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                tracing::info!("Connection from {peer_id}");
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                tracing::info!("Disconnected from {peer_id}");
            }
            other => {
                tracing::trace!("{other:?}");
            }
        }
    }
}
