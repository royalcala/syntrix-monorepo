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
| 6 | Bootstrap nodes | Usar IPFS public bootstrap nodes para que Kademlia no esté aislado. Sin ellos, el DHT nunca encuentra peers no conocidos. |
| 7 | Re-announce en cambio de IP | Al detectar `SwarmEvent::NewListenAddr`, re-anunciar el peer en Kademlia para que otros peers actualicen su dirección. Inspirado en `server.refresh()` de HyperDHT. |
| 8 | Peer blocking a nivel red | `P2PNode::block_peer(peer_id)` que cierra la conexión activa y previene futuros dials. Se usa cuando admin revoca acceso a un device. |
| 9 | Peer scoring para reconexión | Priorizar reconexión a peers con mejor historial: uptime, latencia baja, pocas desconexiones previas. Inspirado en "responsive agents" de Kitsune2. |

## Archivos Afectados

| Archivo | Cambio |
|---|---|
| `crates/syntrix-network/Cargo.toml` | +features: `autonat`, `relay`, `dcutr` |
| `crates/syntrix-network/src/behaviour.rs` | +`autonat`, +`relay::client`, +`dcutr`, +nuevos `CustomBehaviourEvent` variants |
| `crates/syntrix-network/src/lib.rs` | +reconnect loop, +peer scoring, +block_peer, +bootstrap nodes, +re-announce, +Kademlia provider, +TCP listen |
| `apps/client/src-tauri/src/identity.rs` | +reaccionar a `PeerDisconnected`, +NewListenAddr, +discover_org_peers en startup |
| `apps/admin/src-tauri/src/admin.rs` | +llamar `block_peer` al revocar acceso a device |
| `apps/client/src-tauri/Cargo.toml` | +features de syntrix-network |
| `apps/admin/src-tauri/Cargo.toml` | +features de syntrix-network |
| `Cargo.toml` (workspace) | +`crates/syntrix-relay/` (si se implementa tarea 10) |

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

### 5. Bootstrap nodes reales para Kademlia

**Archivo**: `crates/syntrix-network/src/lib.rs`

El `bootstrap_nodes: vec![]` actual deja Kademlia aislado del DHT global. Agregar bootstrap nodes públicos de IPFS:

```rust
let bootstrap_nodes: Vec<Multiaddr> = vec![
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN".parse().unwrap(),
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa".parse().unwrap(),
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb".parse().unwrap(),
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt".parse().unwrap(),
];
```

En `P2PNode::new`, iterar sobre bootstrap nodes:
```rust
for addr in &bootstrap_nodes {
    if let Err(e) = swarm.add_peer_address(local_peer_id, addr.clone()) {
        tracing::warn!("bootstrap add_peer_address failed: {e}");
    }
}
```

Ejecutar `kademlia.bootstrap()` al iniciar y periódicamente (cada 5 min) para mantener la tabla de routing fresca.

### 6. Re-announce en cambio de IP

**Archivo**: `crates/syntrix-network/src/lib.rs`

Cuando la IP del dispositivo cambia (DHCP renewal, WiFi→móvil, VPN connect), los peers pierden la ruta. Agregar en `handle_swarm_event`:

```rust
SwarmEvent::NewListenAddr { address, .. } => {
    // Re-announce in Kademlia so peers can find us at the new address
    if let Some(active_key) = active_kademlia_key.read().clone() {
        swarm.behaviour_mut().kademlia.add_provider(
            active_key,
            *swarm.local_peer_id(),
        );
    }
    // Re-bootstrap to refresh routing table
    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        tracing::warn!("kademlia bootstrap after IP change failed: {e}");
    }
}
```

Mantener `active_kademlia_key` con el `RecordKey` de la org actual, actualizado en `discover_org_peers`.

### 7. Peer blocking a nivel red

**Archivo**: `crates/syntrix-network/src/lib.rs`

Nuevo método en `P2PNode` y campo interno:

```rust
blocked_peers: Arc<RwLock<HashSet<PeerId>>>,
```

```rust
/// Block a peer: close any active connection and prevent future dials.
pub fn block_peer(&self, peer_id: PeerId) {
    self.blocked_peers.write().unwrap().insert(peer_id);
    self.cmd_tx.send(Command::BlockPeer(peer_id)).ok();
}
```

Nuevo comando `Command::BlockPeer(PeerId)` que en el event loop:
```rust
Command::BlockPeer(peer_id) => {
    let _ = swarm.disconnect_peer_id(peer_id);
    // Also cancel any pending reconnect loop for this peer
    reconnect_tasks.remove(&peer_id);
}
```

En `handle_swarm_event`, antes de `ConnectionEstablished`:
```rust
if blocked_peers.read().contains(&peer_id) {
    let _ = swarm.disconnect_peer_id(peer_id);
    return;
}
```

**Archivo**: `apps/admin/src-tauri/src/admin.rs`

Al revocar acceso a un device, llamar:
```rust
state.p2p().block_peer(peer_id);
```

### 8. Peer scoring para reconexión inteligente

**Archivo**: `crates/syntrix-network/src/lib.rs`

Mantener métricas por peer para priorizar reconexión:

```rust
struct PeerScore {
    connected_since: Option<Instant>,
    last_disconnect: Option<Instant>,
    disconnect_count: u32,
    avg_latency_ms: f64,
}
```

Modificar `ensure_connected` para recibir scores:

```rust
pub fn ensure_connected(&self, peer_id: PeerId, addrs: Vec<Multiaddr>, score: Option<PeerScore>) {
    // Higher score → shorter initial backoff delay
    let initial_delay = if let Some(s) = &score {
        if s.disconnect_count < 3 { Duration::from_secs(1) }
        else if s.disconnect_count < 10 { Duration::from_secs(2) }
        else { Duration::from_secs(4) }
    } else {
        Duration::from_secs(2) // default for unknown peers
    };
    // ...
}
```

Tracking de scores en `handle_swarm_event`:
- `ConnectionEstablished` → registrar `connected_since`, resetear `disconnect_count`
- `ConnectionClosed` → registrar `last_disconnect`, incrementar `disconnect_count`
- `Ping` event → actualizar `avg_latency_ms`

### 9. Tests

**Archivo**: `crates/syntrix-network/tests/reconnect_basic.rs`

- Dos `P2PNode` simulando desconexión y reconexión con backoff.
- Verificar que `ensure_connected` restablece la conexión.
- Verificar que peer scoring acelera reconexión de peers estables.

**Archivo**: `crates/syntrix-network/tests/relay_basic.rs`

- Simular conexión vía relay entre dos nodos sin ruta directa.

**Archivo**: `crates/syntrix-network/tests/block_peer.rs`

- Conectar dos peers, llamar `block_peer()`, verificar que la conexión se cierra.
- Verificar que no se puede dialear al peer bloqueado nuevamente.

### 10. Binario `syntrix-relay` (opcional, futura)

Crate separado `crates/syntrix-relay/`:
- Binario standalone con `libp2p::relay::Behaviour` en modo server
- Acepta `--port`, `--keypair` 
- Genera Multiaddr para que peers se conecten
- Sin dependencia en `syntrix-network` — solo libp2p

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Relays IPFS pueden estar caídos o lentos | Múltiples relays en bootstrap. Self-hosting opcional con `syntrix-relay`. |
| dcutr hole punching falla en NATs simétricas | Conexión permanece vía relay (datos encriptados). |
| Backoff puede saturar la red en orgs grandes | Límite de peers por org. Backoff máximo 64s. |
| Kademlia MemoryStore es volátil | Rediscovery vía heartbeats como fallback. |
| Bootstrap nodes IPFS inaccesibles en redes aisladas | Permitir configuración de bootstrap nodes custom vía `NetworkConfig` para entornos offline. |
| `NewListenAddr` puede dispararse muchas veces | Debounce de 5s entre re-announces para evitar spam al DHT. |
| Peer scoring podría reintentar demasiado rápido un peer inestable | `disconnect_count` elevado aumenta el backoff inicial. Peer con >20 desconexiones en 10 min recibe 64s fijo. |

## Validación

1. Compilar con `just build` — los nuevos features de libp2p no deben romper el build.
2. Correr tests existentes: `just test-rust` — sin regresiones en CDC, catchup ni invite.
3. Correr tests E2E: `just test-e2e` — invite, catchup, CDC sync deben funcionar.
4. Test manual: dos peers en redes separadas (o VPN simulando NAT) deben conectarse vía relay.
5. Test manual: desconectar un peer y verificar que se reconecta en <2 minutos.
6. Test manual: peer con IP cambiante debe ser re-descubierto vía Kademlia re-announce.
7. Test manual: `block_peer` en admin debe desconectar al cliente y prevenir reconexión.

## Lo que NO cubre

- `syntrix-relay` binario — tarea separada futura
- `Autonat` server mode (solo client) — solo necesitamos saber si tenemos NAT, no responder consultas
- WebRTC transport — sobreingeniería para desktop-only
- `LlamaProvider` (sin Ollama) — plan separado
- CDC batch size adaptativo — innecesario a escala actual (3-10 peers por org)
- Kademlia `MemoryStore` a disco (persistent peer store) — futura iteración si es necesario
