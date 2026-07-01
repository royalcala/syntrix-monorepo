/home/alcala/Documents/github/syntrix-p2p/syntrix-monorepo/.kilo/worktrees/purple-saltopus/.kilo/plans/1782854204807-iroh-to-libp2p-migration.md# Plan: Migración iroh → libp2p

## Objetivo

Eliminar completamente `iroh` y `iroh-gossip` del monorepo y reemplazarlos con `rust-libp2p`, manteniendo toda la funcionalidad existente (sync P2P, heartbeat, invite, catchup, gossip) con una arquitectura más robusta y mantenible.

## Decisiones de diseño confirmadas

| Decisión | Elección |
|----------|----------|
| Transporte | QUIC + TCP fallback |
| Pub/sub | Gossipsub |
| Descubrimiento | Kademlia DHT |
| Protocolos custom | request_response nativo de libp2p |
| Estructura | Nuevo crate `syntrix-network` |

## Arquitectura resultante

```
crates/
├── syntrix-schema/     # sin cambios
├── syntrix-logging/    # sin cambios
├── syntrix-core/       # modificado: quita iroh, usa tipos de syntrix-network
├── syntrix-network/    # NUEVO: encapsula libp2p Swarm + Behaviours
└── syntrix-testkit/    # actualizado para libp2p

apps/
├── client/src-tauri/   # reescrito: iroh → libp2p vía syntrix-network
└── admin/src-tauri/    # reescrito: iroh → libp2p vía syntrix-network
```

### `syntrix-network` — API pública

```rust
// Swarm gestionado internamente, expone canales de eventos
pub struct P2PNode { /* libp2p::Swarm<CustomBehaviour> */ }

pub struct NetworkConfig {
    pub keypair: Keypair,
    pub listen_addrs: Vec<Multiaddr>,
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub data_dir: PathBuf,
}

impl P2PNode {
    pub async fn new(config: NetworkConfig) -> Result<(Self, EventReceiver)>;

    // Gossipsub
    pub fn join_topic(&mut self, topic: &str) -> Result<()>;
    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> Result<()>;

    // Request-Response (invite + catchup)
    pub fn send_request(&mut self, peer: PeerId, request: Request) -> OutboundRequestId;

    // Peering
    pub fn dial(&mut self, addr: Multiaddr) -> Result<()>;
    pub fn local_peer_id(&self) -> PeerId;
    pub fn listen_addrs(&self) -> Vec<Multiaddr>;
}

pub enum Event {
    GossipsubMessage { topic: String, data: Vec<u8>, source: PeerId },
    RequestReceived { peer: PeerId, request: Request },
    ResponseReceived { request_id: OutboundRequestId, response: Response },
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}
```

### `syntrix-core` — cambios

| Archivo | Cambio |
|---------|--------|
| `addr.rs` | `parse_device_addr()` → acepta `Multiaddr` y extrae `PeerId`. `build_device_addr_string()` → devuelve `Multiaddr` con `/p2p/<peer_id>`. |
| `registry.rs` | `TopicId` de iroh-gossip → `TopicHash` de libp2p gossipsub, o simplemente `String`. |
| `heartbeat.rs` | Sin cambios en interfaz. Internamente usa gossipsub publish. |
| `sync.rs` | Sin cambios. |
| `lib.rs` | Exporta tipos de `syntrix-network` en vez de iroh. |

### Apps — cambios principales

| Módulo | iroh (actual) | libp2p (nuevo) |
|--------|--------------|----------------|
| `identity.rs` | `SecretKey`, `Endpoint`, `Router` | `Keypair`, `P2PNode` |
| `gossip.rs` | `Gossip`, `TopicId`, `subscribe/broadcast` | Gossipsub via `P2PNode` events |
| `invite.rs` | `ProtocolHandler` + ALPN | request_response handler |
| `catchup.rs` | `ProtocolHandler` + ALPN | request_response handler |
| `events.rs` | `gossip_bus.broadcast()` | `p2p_node.publish()` |
| `admin.rs` | `endpoint.connect(peer, ALPN)` | `p2p_node.send_request()` |
| `sync.rs` | sin cambios | sin cambios |
| `indexes.rs` | sin cambios | sin cambios |

## Plan de implementación (ordenado por dependencias)

### Fase 1: `syntrix-network` (crate nuevo)

1. Crear `crates/syntrix-network/Cargo.toml` con dependencias libp2p:
   - `libp2p` con features: `quic`, `tcp`, `dns`, `gossipsub`, `kad`, `identify`, `ping`, `request-response`, `noise`, `yamux`, `macros`, `tokio`
2. Implementar `NetworkConfig` y `P2PNode::new()`:
   - Construir `Swarm` con `SwarmBuilder`
   - Configurar `noise` para encryption
   - Configurar `yamux` para multiplexing
   - Registrar todos los `NetworkBehaviour`
3. Implementar `CustomBehaviour` con `#[derive(NetworkBehaviour)]`:
   - `gossipsub::Behaviour` — pub/sub
   - `kad::Behaviour` — DHT discovery
   - `identify::Behaviour` — peer info
   - `ping::Behaviour` — liveness
   - `request_response::Behaviour<InviteCodec>` — invite protocol
   - `request_response::Behaviour<CatchupCodec>` — catchup protocol
4. Implementar event loop: un `tokio::spawn` que procesa eventos del `Swarm` y los envía por un `mpsc::UnboundedSender<Event>`.
5. Implementar protocolos request-response:
   - `InviteCodec` + `InviteRequest` / `InviteResponse`
   - `CatchupCodec` + `CatchupRequest` / `CatchupResponse`

### Fase 2: `syntrix-core` (modificar)

6. Actualizar `Cargo.toml`: quitar `iroh`, `iroh-gossip`, agregar `syntrix-network`, `libp2p-core`.
7. Reemplazar `addr.rs`:
   - `parse_device_addr(addr_str) -> Option<PeerId>`: parsea Multiaddr o hex PeerId
   - `build_device_addr_string(peer_id, addrs) -> String`: construye Multiaddr con `/p2p/`
8. Reemplazar `registry.rs`:
   - `TopicId` → usar `String` como topic identifier (el hash lo maneja gossipsub internamente)
   - Simplificar `set_topic_id` / `get_topic_id` / `map_topic_to_org` / `lookup_org_by_topic`
9. `heartbeat.rs`: sin cambios en interfaz (sigue usando closures `broadcast` y `store_heartbeat`)
10. Agregar tipos de eventos de red en `lib.rs`

### Fase 3: `syntrix-client` (reescribir capa de red)

11. `identity.rs` / `AppState`:
    - `SecretKey` → `libp2p::identity::Keypair`
    - `Endpoint` → `P2PNode`
    - `Router` → eliminado (la event loop está en P2PNode)
    - `Gossip` → eliminado (gossipsub dentro de P2PNode)
    - Mantener `hlc_counter`, `registry`, `orgs`, `data_dir`, `indexer`, `invite_handler`
    - Agregar `event_rx: mpsc::UnboundedReceiver<Event>`
    - `new_with_data_dir()`: construir `P2PNode::new()` en vez de `Endpoint::builder(N0)`
12. `gossip.rs` / `GossipEventBus`:
    - Reemplazar `Gossip` + `HashMap<String, GossipTopic>` con `P2PNode` event stream
    - `join_org()` → llama `p2p.join_topic(topic_id)` y registra handler en event loop
    - `broadcast()` → llama `p2p.publish()`
    - El event loop central consume `Event::GossipsubMessage` y lo enruta por topic
13. `invite.rs`:
    - Eliminar `ProtocolHandler` de iroh
    - Usar `request_response::Codec` para serializar/deserializar
    - `invite_handler` se convierte en un callback registrado en `P2PNode`
14. `catchup.rs`:
    - Eliminar `ProtocolHandler` de iroh
    - `request_catchup()` → `p2p.send_request(peer, CatchupRequest { org_id, since_hlc })`
    - La respuesta llega como `Event::ResponseReceived`
15. `events.rs`:
    - `commit_event()`: reemplazar `gossip_bus.broadcast()` con `p2p_node.publish(topic, bytes)`
16. `lib.rs`:
    - `run()`: spawnear event loop que procesa `Event` del `P2PNode` y enruta a gossip/catchup/invite handlers
    - Eliminar `block_on(AppState::new())` si es necesario, manejar async correctamente

### Fase 4: `syntrix-admin` (reescribir capa de red)

17. `identity.rs` / `AppState`:
    - Mismos cambios que client: `SecretKey` → `Keypair`, `Endpoint` → `P2PNode`
    - `gossip_bus` cambia a usar `P2PNode` en vez de `iroh_gossip::Gossip`
    - El `db` (redb) se mantiene igual
    - `heartbeats` se mantiene igual (se actualiza desde eventos gossipsub)
18. `gossip.rs` / `AdminGossipBus`:
    - Reemplazar `Gossip` con gossipsub via `P2PNode`
    - `join_org()` → `p2p.join_topic()`
    - `broadcast()` → `p2p.publish()`
    - `write_event_to_redb()` sin cambios
19. `catchup.rs`:
    - Mismos cambios que client: eliminar `ProtocolHandler`, usar request_response
20. `admin.rs`:
    - `send_invite()`: usar `p2p.send_request()` en vez de `endpoint.connect() + open_uni()`
    - `create_org()`: usar topic de gossipsub como String en vez de `iroh_gossip::TopicId::from_bytes()`
    - `build_device_addr_string()` → usa la versión nueva de syntrix-core
21. `lib.rs`: spawn event loop para eventos del P2PNode

### Fase 5: Tests y limpieza

22. `syntrix-testkit`: actualizar helpers para usar `syntrix-network` en vez de iroh
23. Tests e2e: reescribir `apps/admin/src-tauri/tests/e2e_sync_test.rs`
24. Remover `iroh` y `iroh-gossip` de todos los `Cargo.toml`
25. `cargo check` / `cargo test` en todo el workspace
26. Verificar que `cargo build` compila ambos apps

## Mapeo de tipos (cheat sheet para implementación)

| iroh | libp2p |
|------|--------|
| `iroh::SecretKey` | `libp2p::identity::Keypair::generate_ed25519()` |
| `secret.public().as_bytes()` → `[u8; 32]` | `keypair.public().to_peer_id()` |
| `iroh::Endpoint` + `iroh::protocol::Router` | `libp2p::Swarm<CustomBehaviour>` |
| `iroh::EndpointAddr::from_parts(peer, addrs)` | `Multiaddr::from_str("/ip4/.../udp/.../quic-v1/p2p/PeerId")` |
| `endpoint.connect(addr, ALPN)` | `swarm.dial(addr)` |
| `conn.open_uni()` / `accept_uni()` | request_response (automático) |
| `iroh_gossip::net::Gossip::builder().spawn(ep)` | `gossipsub::Behaviour::new()` |
| `gossip.subscribe(topic_id, peers)` | `gossipsub.subscribe(&topic_hash)` |
| `topic.broadcast(bytes)` | `gossipsub.publish(topic_hash, bytes)` |
| `Event::Received(msg)` | `gossipsub::Event::Message { message, .. }` |
| `iroh_gossip::TopicId::from_bytes([u8; 32])` | `gossipsub::IdentTopic::new("syntrix-org-{org_id}")` |
| `iroh::protocol::ProtocolHandler` trait | `request_response::Codec` trait |
| `CaRootsConfig::insecure_skip_verify()` | noise handshake (seguro por defecto) |

## Estado de implementación (2026-06-30)

| Fase | Estado |
|------|--------|
| Fase 1: syntrix-network | ✅ Completo — crate con P2PNode, Event loop, Gossipsub, Kademlia, Identify, Ping, request_response |
| Fase 2: syntrix-core | ✅ Completo — addr.rs con Multiaddr/PeerId, registry.rs con String topics, sin iroh |
| Fase 3: syntrix-client | ✅ Completo — identity.rs, events.rs, lib.rs adaptados a P2PNode |
| Fase 4: syntrix-admin | ✅ Completo — identity.rs, admin.rs, gossip.rs, catchup.rs adaptados a P2PNode |
| Fase 5: Tests | ✅ Parcial — Tests compilan pero requieren red P2P real para ejecutarse |
| Compilación | ✅ `cargo check` pasa en todo el workspace |

## Riesgos y mitigaciones

| Riesgo | Mitigación |
|--------|-----------|
| libp2p API compleja, curva de aprendizaje | Encapsular TODO en `syntrix-network`, apps solo usan API simple |
| Gossipsub tiene más overhead que floodsub | Escalar bien con mesh overlay, es el estándar de la industria |
| Kademlia requiere bootstrap nodes | Proveer nodos bootstrap por defecto (IPFS public ones o propios) |
| QUIC puede fallar en firewalls corporativos | TCP fallback automático (`tcp` feature) |
| Migración de claves (Keypair formato diferente) | Generar nuevas claves ed25519, persistir en `keypair.bytes`. Los PeerId cambian, requiere reinvitar dispositivos. |
| Cambio de PeerId rompe invites existentes | Esto es inevitable. Los dispositivos existentes necesitarán nuevos invites del admin. Documentar en release notes. |

## Lo que NO cambia

- `syntrix-schema` completo (schemas, upcasters, registry, encoded)
- `syntrix-logging` completo
- `RelationalEngine` / `indexes.rs` (redb, tantivy, índices)
- `sync.rs` (sync_push, sync_pull, sync_status, get_sync_info)
- `audit.rs` (ambos)
- `search.rs`
- `events.rs` (lógica de HLC y commit_event, solo cambia el broadcast)
- `seed.rs` (dev data)
- Frontends (Tauri UI), package.json, pnpm
- `justfile`
- `NamespaceRegistry` (solo cambia `TopicId` → `String`)
- Persistencia JSON (`orgs.json`, `devices.json`, `roles.json`)
- redb databases
