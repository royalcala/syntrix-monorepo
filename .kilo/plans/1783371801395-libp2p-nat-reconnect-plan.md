# Plan: libp2p NAT Traversal + Auto-Reconexión

## Objetivo

Cerrar los 4 gaps de networking que impiden conectividad confiable entre peers detrás de NAT y recuperación ante desconexiones.

## Decisiones de Diseño

| # | Decisión | Elección |
|---|---|---|
| 1 | Relay para NAT traversal | Relays públicos IPFS como default. Binario `syntrix-relay` opcional para self-hosting. |
| 2 | Hole punching | `dcutr` (Direct Connection Upgrade Through Relay) — intenta conexión directa tras el relay, evitando pasar datos por el relay. |
| 3 | Auto-reconexión | Backoff exponencial (2s → 4s → 8s → ... → 64s). Rediscovery vía Kademlia + heartbeats. |
| 4 | Transporte adicional | TCP (además del QUIC existente) como fallback para firewalls que bloquean UDP. |
| 5 | Kademlia peer discovery | `add_provider`/`get_providers` sobre `hash("syntrix-p2p-" + org_id)` para que peers de la misma org se encuentren sin depender solo de heartbeats. |

## Archivos Afectados

| Archivo | Cambio |
|---|---|
| `crates/syntrix-network/Cargo.toml` | +features: `autonat`, `relay`, `dcutr` |
| `crates/syntrix-network/src/behaviour.rs` | +`autonat::Behaviour`, +`relay::client::Behaviour`, +`dcutr::Behaviour` |
| `crates/syntrix-network/src/lib.rs` | +reconnect loop, +peer address cache, +Kademlia provider methods, +TCP listen |
| `apps/client/src-tauri/src/identity.rs` | +reaccionar a `PeerDisconnected` iniciando reconexión |
| `apps/client/src-tauri/Cargo.toml` | +features de syntrix-network |
| `apps/admin/src-tauri/Cargo.toml` | +features de syntrix-network |
| `Cargo.toml` (workspace) | +`crates/syntrix-relay/` (si se implementa tarea 6) |

## Tareas

### 1. Habilitar autonat + relay client + dcutr en syntrix-network

**Archivo**: `crates/syntrix-network/Cargo.toml`

```toml
libp2p = { version = "0.54", features = [
    "quic", "tcp", "dns", "gossipsub", "kad", "identify", "ping",
    "request-response", "noise", "yamux", "macros", "tokio",
    "autonat", "relay", "dcutr",   # ← nuevos
] }
```

**Archivo**: `crates/syntrix-network/src/behaviour.rs`

Agregar a `CustomBehaviour`:
```rust
pub struct CustomBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub rr: request_response::Behaviour<CombinedCodec>,
    pub autonat: autonat::Behaviour,              // nuevo
    pub relay_client: relay::client::Behaviour,    // nuevo
    pub dcutr: dcutr::Behaviour,                   // nuevo
}
```

Agregar `CustomBehaviourEvent` variants: `Autonat`, `RelayClient`, `Dcutr`.

**Archivo**: `crates/syntrix-network/src/lib.rs`

1. Configurar relay client con IPFS bootstrap relays:
```rust
let relay_config = relay::client::Config::default();
let relay_client = relay::client::Behaviour::new(
    peer_id,  // local peer id
    relay_config,
);
```

2. Agregar a `SwarmBuilder` en `P2PNode::new`:
```rust
.with_relay_client(NoiseConfig::new, yamux::Config::default)?
```

3. Manejar eventos `RelayClient` en `handle_swarm_event`:
   - `relay::client::Event::ReservationReqAccepted` → logging y ready para relay
   - `relay::client::Event::CircuitReqAccepted` → relay establecido

4. Manejar eventos `Dcutr` — hole punching automático vía relay.

**Bootstrap relays**: usar IPFS defaults de libp2p en lugar de `bootstrap_nodes: vec![]`.

### 2. TCP como transporte fallback

**Archivo**: `crates/syntrix-network/src/lib.rs`

Agregar a `listen_on` en `P2PNode::new`:
```rust
let listen_on: Vec<Multiaddr> = vec![
    "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
    "/ip4/0.0.0.0/tcp/0".parse().unwrap(),   // ← nuevo
];
```

Y en `SwarmBuilder`:
```rust
.with_tcp_config(tcp::Config::default().port_reuse(true))
```

**Archivo**: `apps/client/src-tauri/src/identity.rs` y `apps/admin/src-tauri/src/identity.rs`

Mismo cambio en sus `listen_on`.

### 3. Auto-reconexión con backoff exponencial

**Archivo**: `crates/syntrix-network/src/lib.rs`

Agregar a `P2PNode`:
```rust
// Nuevos campos internos
connected_peers: Arc<RwLock<HashSet<PeerId>>>,
```

Nuevo método público:
```rust
pub fn ensure_connected(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
    // Dispara tarea tokio con backoff
}
```

Lógica de reconexión (función interna):
```
fn reconnect_loop(swarm_tx, peer_id, addrs):
    for delay in [2s, 4s, 8s, 16s, 32s, 64s]:
        if already_connected(peer_id): break
        try dial(addrs) + kademlia_lookup(peer_id)
        sleep(delay)
```

En `handle_swarm_event`:
- `ConnectionEstablished` → marcar peer como conectado, cancelar backoff
- `ConnectionClosed` → iniciar backoff loop para ese peer

**Archivo**: `apps/client/src-tauri/src/identity.rs`

En `process_event_loop`, reemplazar:
```rust
// Antes:
Event::PeerConnected(_) | Event::PeerDisconnected(_) => {}

// Después:
Event::PeerDisconnected(peer_id) => {
    if let Some(addrs) = get_stored_peer_addrs(peer_id) {
        let _ = p2p.ensure_connected(peer_id, addrs);
    }
}
// PeerConnected se maneja en P2PNode internamente
```

### 4. Kademlia peer discovery por org

**Archivo**: `crates/syntrix-network/src/lib.rs`

Nuevo método en `P2PNode`:
```rust
/// Anuncia este peer como proveedor para una org, y busca otros peers de la org.
pub fn discover_org_peers(&self, org_id: &str) -> Vec<PeerId> {
    let key = kad::RecordKey::new(&blake3::hash(format!("syntrix-p2p-{}", org_id).as_bytes()));
    // add_provider → announce self
    // get_providers → find other peers
}
```

**Archivo**: `apps/client/src-tauri/src/identity.rs`

Al unirse a una org (`join_org_impl`) y al cargar orgs en startup:
```rust
let org_peers = p2p.discover_org_peers(&final_org_id);
for peer_id in org_peers {
    if peer_id != local_peer_id {
        p2p.ensure_connected(peer_id, vec![]); // addrs via identify + kademlia
    }
}
```

### 5. Tests

**Archivo**: `crates/syntrix-network/tests/reconnect_basic.rs`

- Dos `P2PNode` en procesos separados simulando desconexión y reconexión
- Verificar que `ensure_connected` restablece la conexión dentro del timeout de backoff

**Archivo**: `crates/syntrix-network/tests/relay_basic.rs` (opcional si hay infra de relay)

- Simular conexión vía relay entre dos nodos sin ruta directa

### 6. Binario `syntrix-relay` (opcional, futura)

Crate separado `crates/syntrix-relay/`:
- Binario standalone con `libp2p::relay::Behaviour` en modo server
- Acepta `--port`, `--keypair` 
- Genera Multiaddr para que peers se conecten
- Sin dependencia en `syntrix-network` — solo libp2p

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Relays IPFS pueden estar caídos o lentos | Múltiples relays en bootstrap. Self-hosting opcional. |
| dcutr hole punching falla en NATs simétricas | Conexión permanece vía relay (datos encriptados). |
| Backoff puede saturar la red en orgs grandes | Límite de peers por org. Backoff máximo 64s. |
| Kademlia MemoryStore es volátil | Rediscovery vía heartbeats como fallback. |

## Validación

1. Compilar con `just build` — los nuevos features de libp2p no deben romper el build.
2. Correr tests existentes: `just test-rust` — sin regresiones.
3. Correr tests E2E: `just test-e2e` — invite, catchup, CDC sync deben funcionar.
4. Test manual: dos peers en redes separadas (o VPN simulando NAT) deben conectarse vía relay.
5. Test manual: desconectar un peer y verificar que se reconecta en <2 minutos.

## Lo que NO cubre

- `syntrix-relay` binario — tarea separada futura
- `Autonat` server mode (solo client) — solo necesitamos saber si tenemos NAT, no responder consultas
- WebRTC transport — sobreingeniería para desktop-only
- `LlamaProvider` (sin Ollama) — plan separado
