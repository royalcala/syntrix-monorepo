# Plan: Hardening de Conectividad P2P + Arquitectura Móvil (Anchor/Hub)

> **Preámbulo**: ADR `1783695719135-adr-libp2p-vs-headscale.md` y `1783695719137-adr-libp2p-vs-zenoh.md`.
> **Estado**: **Parte A** (hardening) en implementación (P0/P1 ya comenzaron). **Parte B** (arquitectura móvil) es diseño aprobado, se ejecuta después de Parte A.
> **Fecha**: 2026-07-10.
> **Ámbito**: `crates/syntrix-network`, `apps/syntrix-relay`, `apps/{client,admin}`, capa de transporte.
>
> Este documento tiene dos partes:
> - **Parte A — Hardening de conectividad P2P** (abajo): bugs/gaps de red a cerrar sobre el stack libp2p actual. En curso.
> - **Parte B — Arquitectura móvil Anchor/Hub + re-evaluación de transporte** (al final): decisión estratégica para mobile-primary. Cambia la topología a hub-and-spoke y reabre la elección de transporte a largo plazo.

## Diagnóstico: lo que está roto o ausente hoy

Tras revisar el código, estos son los gaps encontrados — algunos son bugs, otros son features no habilitadas.

| # | Hallazgo | Severidad | Archivo:línea |
|---|---|---|---|
| 1 | **Sin `mdns`** — la LAN pura no descubre peers automáticamente | Crítica (LAN offline) | `syntrix-network/Cargo.toml:8` (falta feature), `behaviour.rs` (falta Behaviour) |
| 2 | **`bootstrap_nodes: vec![]` en producción** — Kademlia jamás conoce otros peers si no los dialean explícitamente | Crítica (NAT/discovery) | `identity.rs:83` (client), `identity.rs` (admin) |
| 3 | **Sin `listen_on /p2p-circuit`** — el relay client nunca obtiene una reservation, nunca se anuncia ni se usa | Crítica (NAT traversal) | `identity.rs:75-78` |
| 4 | **`discover_org_peers` retorna `vec![]` inmediatamente** sin esperar la respuesta de Kademlia | Alta (discovery) | `lib.rs:323-330` |
| 5 | **`ensure_connected` diala `Multiaddr::empty()`** sobrescribiendo las direcciones reales que recibe | Alta (reconexión) | `lib.rs:309`, `lib.rs:379` |
| 6 | **`add_peer_address` usa `local_peer_id` en vez del peer_id del nodo bootstrap** | Alta (bootstrap roto) | `lib.rs:153` |
| 7 | **`autonat` y `dcutr` declarados dos veces** — código muerto sombrea la primera inicialización | Baja (desperdicio) | `lib.rs:117-123` |
| 8 | **`CatchupRequestReceived` todavía usa `query_events_since` (event_log legacy)** en vez de snapshot CDC | Media (catch-up legacy) | `estado-actual.md:104`, `catchup.rs` |
| 9 | **Kademlia `MemoryStore` es volátil** — routing table se pierde al reiniciar, cada restart arranca desde cero | Media (discovery frío) | `lib.rs:100` |
| 10 | **Sin sistema de heartbeats/presencia real** — no se escribe heartbeat periódico, `process_event_loop` no maneja `PeerDisconnected` con reconexión | Media (presencia) | `estado-actual.md:102` |
| 11 | **Grid sin virtualización** — baja performance >1000 filas | Baja (pero en prioridades) | `estado-actual.md:103` |

## Tareas (priorizadas)

### P0 — Críticos (sin esto el P2P no funciona en producción)

#### T1: Habilitar `mdns` para descubrimiento LAN

**Archivos**: `crates/syntrix-network/Cargo.toml`, `src/behaviour.rs`, `src/lib.rs`

- Feature: agregar `"mdns"` al feature set de libp2p.
- `CustomBehaviour`: agregar `mdns: mdns::tokio::Behaviour`.
- `CustomBehaviourEvent`: agregar variante `Mdns(mdns::Event)`.
- `handle_swarm_event`: reaccionar a `Mdns(MdnsEvent::Discovered(list))` → dialear peers descubiertos (si no son el local y no están bloqueados).
- No se necesita configuración extra — mDNS funciona automáticamente en la misma subred.

**Validación**: dos peers en misma LAN (sin internet) se descubren y conectan en <2s.

#### T2: Bootstrap nodes y relay reservation (cierra NAT traversal real)

**Archivos**: `apps/client/src-tauri/src/identity.rs`, `apps/admin/src-tauri/src/identity.rs`

- `listen_on`: agregar `"/ip4/0.0.0.0/udp/0/quic-v1/p2p-circuit"` para que el relay client pueda obtener una reservation.
- `bootstrap_nodes`: reemplazar `vec![]` con al menos los bootstrap nodes públicos de IPFS:
  ```
  "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN".parse().unwrap(),
  "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa".parse().unwrap(),
  "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb".parse().unwrap(),
  "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt".parse().unwrap(),
  ```
  Idea: agregar `#[allow(dead_code)] fn default_bootstrap_nodes() -> Vec<Multiaddr>` en `syntrix-network` y usarla desde ambos identity.rs cuando `bootstrap_nodes` viene vacío.
- `NetworkConfig`: hacer `bootstrap_nodes` opcional (`Vec<Multiaddr>`) o usar la función default si está vacío.

**Archivo**: `crates/syntrix-network/src/lib.rs`

- **Fix**: `add_peer_address` en línea 153 usa `local_id` (`*swarm.local_peer_id()`) pero debería parsear el PeerId del multiaddr del bootstrap node. Corregir a:
  ```rust
  if let Some(peer_id) = addr.iter().find_map(|p| {
      if let libp2p::multiaddr::Protocol::P2p(hash) = p {
          PeerId::from_multihash(hash).ok()
      } else { None }
  }) {
      swarm.add_peer_address(peer_id, addr.clone());
  }
  ```
- Bombardear Kademlia bootstrap después de agregar las direcciones.

**Validación**: peer sin conocimiento previo de otros peers descubre y se conecta vía Kademlia + relay.

#### T3: Fix `discover_org_peers` (Kademlia providers async real)

**Archivo**: `crates/syntrix-network/src/lib.rs:323-330`

El comando `DiscoverOrgPeers` hoy envía `result_tx.send(vec![])` inmediatamente sin esperar la respuesta real de `get_providers`. El resultado real llega como `Kademlia(KadEvent::OutboundQueryProgressed{GetProviders{providers}})` en el event loop.

Re-diseño:
- `Command::DiscoverOrgPeers` guarda `result_tx` en un `HashMap<kad::QueryId, oneshot::Sender<Vec<PeerId>>>` (similar a como se usa `pending_catchup`).
- El query ID se obtiene del retorno de `get_providers`.
- `handle_swarm_event` recibe `Kademlia(KadEvent::OutboundQueryProgressed{id, GetProviders{Ok(GetProvidersOk{providers})}})` → busca el `result_tx` por query ID y envía los peers.

**Validación**: `discover_org_peers("test-org").await` retorna peers reales, no vacío.

#### T4: Fix `ensure_connected` y `reconnect_loop` (dialean `Multiaddr::empty()`)

**Archivo**: `crates/syntrix-network/src/lib.rs`

El problema es doble:
1. `ensure_connected` recibe `addrs` pero en `Command::EnsureConnected` el closure que spawn ea `reconnect_loop` recibe los addrs correctos, pero `reconnect_loop` nunca los usa — solo dialea `Multiaddr::empty()` en la línea 379.
2. En `identity.rs`, `ensure_connected` se llama con `vec![]` (línea 415 client, 634 admin).

Correcciones:
- `reconnect_loop`: usar las direcciones reales (`addrs`) en lugar de `Dial(Multiaddr::empty())`. Si `addrs` está vacío, intentar Kademlia lookup (`discover_org_peers` para la org relevante) antes de dialear.
- `identity.rs`: en el handler de `PeerDisconnected`, obtener las direcciones conocidas del peer (via `identify` o almacenadas de conexiones previas) y pasarlas a `ensure_connected`. Si no hay addrs, pasar `vec![]` y dejar que `reconnect_loop` haga Kademlia lookup.
- `P2PNode`: mantener un campo interno `peer_addrs: Arc<RwLock<HashMap<PeerId, Vec<Multiaddr>>>>` que se actualiza en `ConnectionEstablished` (con el `endpoint`/`remote_addr` de la conexión) y en `Identify(Identified{listen_addrs})`.

**Validación**: desconectar un peer manualmente y verificar que `ensure_connected` lo reconecta vía las direcciones almacenadas o vía Kademlia lookup.

### P1 — Altos (funcionalidad importante sin la cual el sistema es frágil)

#### T5: Limpiar `autonat`/`dcutr` duplicados

**Archivo**: `crates/syntrix-network/src/lib.rs:117-123`

Eliminar líneas 117-119 (primera declaración) y dejar solo 121-123 (que es la que se pasa al SwarmBuilder). Son código muerto que no afecta comportamiento pero es confuso.

#### T6: Reparar catch-up para usar snapshot CDC en vez de event_log legacy

**Archivos**: `apps/client/src-tauri/src/catchup.rs`, `syntrix-network/src/lib.rs` (handler `CatchupRequestReceived`)

`estado-actual.md:104` documenta este gap. El admin hoy responde con `query_events_since` (event_log legacy) pero el event_log del admin ya solo es auditoría (`row_image` ya no tiene la forma `{type, hlc, payload}`), así que un peer nuevo no recibe datos históricos completos.

Cambio:
- Admin: al recibir `CatchupRequest`, llamar a `syntrix_network::cdc::snapshot_org_rows()` (que lee el estado actual de todas las tablas de la org) y devolverlo como batch CDC + roster completo.
- Cliente: el path de `apply_catchup_event` (en catchup.rs) ya procesa batches CDC — solo hay que cambiar lo que envía el admin.

**Validación**: peer nuevo (sin datos) hace catch-up y obtiene todas las filas actuales.

#### T7: Kademlia store persistente

**Archivo**: `crates/syntrix-network/src/lib.rs`

`MemoryStore::new(local_peer_id)` se pierde al reiniciar. Esto hace que cada restart sea un "cold start" de Kademlia (sin pares conocidos, sin routing table).

Opciones:
- **Opción A (rápida)**: `kad::store::MemoryStore` con población inicial desde un archivo JSON de peers conocidos. Guardar cada vez que `MemoryStore` cambia (modo periódico, cada 60s).
- **Opción B (ideal, más trabajo)**: implementar un `kad::store::RecordStore` sobre Limbo (tabla `kademlia_store`) para persistencia real en SQL.

Recomiendo Opción A como P1 (rápida y suficiente para 3-10 peers), y Opción B diferida a futuro si la escala lo justifica.

**Validación**: reiniciar un peer y verificar que la routing table de Kademlia contiene los peers de la sesión anterior (no arranca vacía).

#### T8: Sistema de heartbeats/presencia

**Archivos**: `apps/{client,admin}/src-tauri/src/identity.rs`

`estado-actual.md:102` lo marca como prioridad inmediata. Sin heartbeats:
- No se sabe qué peers están online para catch-up alternativo.
- No se puede mostrar estado en UI (`SyncStatusIndicator`).
- `PeerDisconnected` no dispara reconexión (depende de T4 para ser útil).

Implementación:
- Cada 15s, escribir `{ts, status: "online", node_id}` en tabla `heartbeats` local.
- Publicar heartbeat en topic gossipsub de la org para que otros peers sepan que este nodo está vivo.
- En `process_event_loop`, al recibir heartbeat de otro peer, actualizar `peer_heartbeats: HashMap<PeerId, Instant>`.
- Exponer comando `get_sync_info` (ya planeado) que usa estos datos para determinar peers online/offline.

**Validación**: `get_sync_info()` retorna la lista correcta de peers online/offline.

### P2 — Medios (mejoras de calidad y flujo)

#### T9: Virtualización del Grid

**Archivo**: `packages/syntrix-ui/src/EntityGrid.tsx`

`estado-actual.md:103`. Usar `@tanstack/react-virtual` para virtualizar filas en el grid. Mantener scroll a 60 FPS con miles de registros.

#### T10: Habilitar relay self-hosted como feature de producción

**Archivo**: `apps/syntrix-relay/src/main.rs`

El binario existe y compila, pero:
- La keypair es efímera (`generate_ed25519()` en vez de cargada de disco), lo que implica que la dirección del relay cambia en cada restart.
- No está integrado en el flujo de deploy (no hay `just relay` ni documentación de cómo levantarlo).

Cambios:
- `syntrix-relay`: aceptar `--keypair-file` para persistir la keypair y mantener PeerId estable.
- `justfile`: agregar target `just relay` que compile y ejecute.
- Documentar la multiaddr del relay para que peers puedan configurarlo.

#### T13: Inversión del flujo de enrolamiento (cliente inicia conexión al admin)

> **Depende de**: P0 completo (T1–T4). Sin Kademlia funcional ni mdns, el admin no re-descubrirá al cliente si cambia de IP; pero eso es el mismo problema actual y no bloquea esta mejora.
> **Objetivo**: eliminar la necesidad de que el admin conozca la dirección del cliente antes de enviar el invite. Solo se transporta la dirección del admin (QR/ticket), no la del cliente.

**Motivación**: hoy el flujo requiere que el admin tenga el `device_addr` del cliente antes de poder enviar el invite. En la práctica esto significa copiar una multiaddr larga del cliente y pegarla en el admin. Con esta mejora, el cliente inicia la conexión al admin (cuya dirección ya tiene vía QR) y se presenta como "solicitante de enrolamiento". El admin solo aprueba.

**Flujo propuesto**:

```
1. Admin → get_invite_info(org) → QR/ticket con admin_addr (igual que hoy)
2. Cliente → escanea QR → obtiene admin PeerId + direcciones
3. Cliente → dial(admin) + envía mensaje "enroll_request" con su PeerId + node_id
   (nuevo protocolo request_response: /syntrix/enroll/1)
4. Admin UI → recibe enroll_request → muestra "Dispositivo X solicita enrolamiento"
5. Admin → aprueba, asigna rol → envía invite de vuelta por la misma conexión
6. Cliente → recibe invite, hace catch-up normalmente
```

**Archivos afectados**:
- `crates/syntrix-network/src/codecs.rs`: nuevo request type `EnrollRequest { node_id, peer_id }` y response `EnrollResponse { accepted, invite_payload, error }`.
- `crates/syntrix-network/src/lib.rs`: nuevo protocolo `/syntrix/enroll/1` en el `request_response::Behaviour`, manejar `EnrollRequest` en `handle_swarm_event` emitiendo `Event::EnrollRequestReceived`.
- `apps/admin/src-tauri/src/identity.rs`: handler de `EnrollRequestReceived` → almacenar en lista de "pending enrollments", exponer vía Tauri command `get_pending_enrollments(org)` / `approve_enrollment(peer_id, role, can_open, can_write)` / `reject_enrollment(peer_id)`.
- `apps/client/src-tauri/src/identity.rs`: al unirse a org, si tiene `admin_addr`, dialear al admin y enviar `EnrollRequest` antes de hacer catch-up. Esperar `EnrollResponse` con timeout. Si es aceptado, proceder con catch-up.
- `apps/admin/src` (frontend): UI para ver y aprobar/rechazar solicitudes de enrolamiento pendientes.

**Validación**: cliente fuera de LAN escanea QR del admin, envía enroll_request, admin aprueba desde UI, cliente recibe invite y sincroniza — sin que el admin haya tenido que copiar ninguna dirección del cliente.

### P3 — Bajos (mejoras laterales)

#### T11: Grid avanzado — columnas redimensionables/ocultables, multi-selección

`estado-actual.md:96`. Mejora de UX, no bloqueante.

#### T12: `SyncStatusIndicator` en la UI

Componente visual en la barra lateral mostrando estado P2P real (online/offline, peers conectados, docs pendientes de sync). Depende de T8 (heartbeats).

---

## Archivos afectados (resumen)

```
crates/syntrix-network/
├── Cargo.toml                    ← +mdns feature
├── src/behaviour.rs               ← +mdns Behaviour, +Mdns event variant
├── src/lib.rs                     ← fix bootstrap add_peer_address, fix discover_org_peers,
│                                    fix reconnect_loop, fix ensure_connected,
│                                    limpiar autonat/dcutr duplicados,
│                                    +persist para Kademlia store,
│                                    +peer_addrs HashMap
└── tests/                         ← nuevos tests de integración (opcional)

apps/client/src-tauri/src/
├── identity.rs                    ← +bootstrap nodes, +p2p-circuit listen, +heartbeats,
│                                    +PeerDisconnected → ensure_connected, +NewListenAddr
├── catchup.rs                     ← migrar a snapshot CDC
└── cdc_sync.rs                    ← sin cambios (ya funcional)

apps/admin/src-tauri/src/
├── identity.rs                    ← +bootstrap nodes, +p2p-circuit listen, +heartbeats
└── admin.rs                       ← sin cambios (block_peer ya está)

apps/syntrix-relay/src/
└── main.rs                        ← persistir keypair, aceptar --keypair-file
```

---

## Riesgos

| Riesgo | Mitigación |
|---|---|
| mDNS puede generar ruido en redes grandes | Solo se dialean peers de nuestro protocolo; el tráfico es mínimo (LAN multicast) |
| Bootstrap nodes IPFS pueden ser inaccesibles en redes corporativas | Permitir override vía `NetworkConfig`; el fallback es `mdns` + relay + dial directo |
| `discover_org_peers` async real puede colgarse en redes lentas | Timeout de 10s en el oneshot, con fallback a vec vacío |
| Persistencia de Kademlia store puede desincronizarse | Es best-effort; el cold-start se mitiga con bootstrap nodes + mDNS |
| `reconnect_loop` puede saturar con muchos intentos en corto tiempo | El backoff existente (2s→64s) se mantiene; solo se añaden direcciones reales |

## Validación final

1. `just build` — compila todo sin errores ni warnings nuevos.
2. `just test-rust` — sin regresiones en tests existentes (CDC, catchup, invite, block_peer, relay_basic, reconnect_basic).
3. `just test-e2e` — invite, catchup, CDC sync deben funcionar.
4. Manual: dos peers en misma LAN (sin internet) se descubren vía mDNS y sincronizan CDC.
5. Manual: un peer detrás de NAT de operador móvil se conecta vía relay (`/p2p-circuit`) a un peer con IP pública o con relay reservation.
6. Manual: desconexión y reconexión automática en <2 minutos (vía `ensure_connected` con direcciones reales o Kademlia lookup).
7. Manual: reiniciar un peer y verificar que redescubre sus peers conocidos (vía Kademlia persistido o bootstrap + mDNS).

---

# Parte B — Arquitectura Móvil (Anchor/Hub) + re-evaluación de transporte

## B.0 Contexto y premisa

Se asume que **la mayoría de instalaciones serán móviles** (Android/iOS), tanto para admins como para empleados. Hoy las apps son desktop-only (`gen/schemas` solo desktop/linux; existe el stub `#[cfg_attr(mobile, tauri::mobile_entry_point)]` pero sin build móvil).

Los tres muros de mobile P2P obligan a cambiar la topología:

1. **Background execution**: iOS suspende ~30s tras background; Android mata servicios (Doze). Un móvil **no puede ser peer siempre-activo**. Rompe "Sincronía Orgánica en background" y "nodo de backup en teléfono".
2. **NAT móvil (CGNAT/simétrico)**: hole punching (dcutr) falla seguido → móvil-a-móvil directo poco fiable → **relay obligatorio**.
3. **Permisos de red local**: mDNS/multicast en iOS requiere entitlement especial de Apple; en Android requiere `MulticastLock`. LAN discovery queda gated.

## B.1 Decisiones tomadas (aprobadas / delegadas)

| # | Decisión | Elección |
|---|---|---|
| D1 | Postura de infraestructura móvil | **Hub + relay siempre-activos.** Móviles = hojas intermitentes que sincronizan en foreground. El hub puede ser hardware del cliente (soberanía intacta) u hosteado por Syntrix como servicio. |
| D2 | Rol de autoridad del hub | **Ancla headless con llave admin.** El nodo siempre-activo corre el backend admin headless y porta la llave admin: autoridad + catch-up + roster + relay, 24/7. Reusa el path `admin-headless` existente. |
| D3 | Canal de control del admin humano | **Control remoto por libp2p**: protocolo nuevo `/syntrix/admin-control/1` (request_response). El teléfono del admin tiene su propia llave, emparejada una vez con el ancla (QR). El ancla ejecuta ops privilegiadas; la **llave admin raíz nunca sale del ancla**. |
| D4 | Estructura de apps | **Deprecar la app admin standalone.** Una **app unificada role-aware** para usuarios finales (empleado y admin instalan lo mismo; el rol decide qué ve). El backend de autoridad se mueve al **ancla headless** (artefacto nuevo, = headless admin + relay + endpoint público). |
| D5 | Replicación en móvil | **Replicación con alcance (scoped).** El móvil NO replica toda la org (no escala a 1M+ filas en un teléfono). Replica solo lo que su rol/permisos permiten + datos recientes/relevantes; consulta al ancla on-demand para el resto. |
| D6 | Sync en background | **Foreground-sync por defecto.** Push (APNs/FCM) **opt-in** como conveniencia (wake-to-sync), marcado como no-soberano (pasa por Apple/Google + requiere gateway de Syntrix). No es requisito. |
| D7 | Transporte (corto plazo) | **Mantener libp2p** para el lanzamiento móvil. La decisión D1 (ancla con endpoint público) convierte la topología en **hub-and-spoke** y elimina el problema NAT más difícil: los móviles se conectan *hacia* el ancla (outbound, sobrevive CGNAT), sin hole punching. |
| D8 | Transporte (largo plazo) | **Introducir un trait de abstracción de transporte** y prototipar **Zenoh** detrás de él como candidato v2. Ver B.4. |

## B.2 Topología objetivo

```
   [App unificada — Empleado]   [App unificada — Admin (UI)]   [App unificada — Empleado]
         │ scoped sync                │ /syntrix/admin-control/1        │ scoped sync
         │ (foreground)               │ (remote control)                │ (foreground)
         └────────────────┬──────────┴────────────────┬────────────────┘
                          │  (conexión OUTBOUND al ancla, E2E, sobrevive CGNAT)
                 ┌────────┴─────────────────────────────────┐
                 │  ANCLA HEADLESS (por org)                 │
                 │  = headless admin (llave admin, autoridad)│
                 │  + catch-up responder + roster            │
                 │  + relay (circuit-relay-v2)               │
                 │  + réplica autoritativa completa (Turso)  │
                 │  en PC del cliente o VPS (endpoint público / port-forward) │
                 └───────────────────────────────────────────┘
```

- Móviles = hojas intermitentes con réplica **scoped**; escriben offline-first en Limbo local y sincronizan al ancla en foreground.
- Ancla = fuente de verdad de la org, siempre-activa, con endpoint alcanzable.
- P2P directo móvil-a-móvil (mismo LAN, sin internet) queda como caso borde cubierto por mDNS (Parte A, T1); no es el camino principal.

## B.3 Re-evaluación del transporte (¿sigue siendo ideal libp2p?)

**La decisión D1 cambia la premisa de los ADR previos.** Esos ADR concluyeron "mantener libp2p" bajo el supuesto de **P2P puro sin servidor**. Al aceptar un **ancla siempre-activa (hub-and-spoke)**, el análisis cambia:

- **libp2p**: sigue funcionando bien. En hub-and-spoke con ancla de endpoint público, los móviles conectan hacia el ancla (sin hole punch), gossipsub sirve para fan-out del ancla a hojas online, request_response para catch-up/queries scoped. Kademlia/mesh pierden centralidad pero no estorban. **Es el camino de menor riesgo y ya está construido.** → **Elección para el lanzamiento.**
- **Zenoh**: sus dos objeciones del ADR **se neutralizan** en esta topología:
  - "Router en data-path" era el gran contra → pero **ya decidimos tener un ancla en el data-path**. El router Zenoh = nuestra ancla. No agrega servidor.
  - Su modelo nativo *es* client→router (hub-and-spoke), que sobrevive CGNAT igual que nuestro plan. Sus **key expressions** (`org/{id}/{entity}/**`) + **queryables** encajan perfecto con la **replicación scoped (D5)**: un móvil se suscribe a su slice y hace `get` on-demand del resto. Sus **storages** en el ancla dan catch-up/historia nativos.
  - → **Zenoh pasa a ser el candidato líder para transporte v2**, no descartado.
- **Headscale/Tailscale**: sigue siendo solo underlay opcional; con ancla de endpoint público, innecesario para el caso común.

**Conclusión**: libp2p ahora, con un **trait de abstracción de transporte** para no quedar acoplados, y Zenoh como v2 a prototipar detrás del trait cuando la replicación scoped y las orgs grandes lo justifiquen. Actualizar el ADR de Zenoh (`1783695719137`) para reflejar que bajo hub-and-spoke la balanza se acerca.

## B.4 Tareas (Parte B) — ordenadas

> Dependen de que Parte A (P0) esté completa. No iniciar antes.

### BM0 — Portabilidad móvil base (bloqueante de todo lo demás)
- Reemplazar `dirs_next::data_dir()` por el path resolver de Tauri (app data dir del sandbox) en `apps/{client,admin}/src-tauri/src/{lib,identity}.rs`. Sin esto la app no arranca en Android/iOS.
- Verificar que `turso_core` (Limbo) abre/escribe en el sandbox móvil.
- Configurar targets Tauri mobile (Android/iOS): `tauri android init`, `tauri ios init`, capabilities/entitlements base.
- Ajustar `keypair.bytes` / `orgs.json` / `known_peers.json` a rutas del sandbox.

### BM1 — Ancla headless como artefacto de deployment
- Empaquetar el `admin-headless` existente como binario de ancla desplegable (Docker + binario), con `--data-dir`, `--keypair-file`, listen público (QUIC+TCP) y **relay behaviour habilitado** (fusionar rol de `syntrix-relay` en el ancla o correrlos juntos).
- Endpoint estable: documentar QUIC+TCP público / port-forward; opción VPS.
- Salud/observabilidad mínima (logs, endpoint de status).

### BM2 — Canal de control admin remoto (`/syntrix/admin-control/1`)
- Nuevo request/response en `syntrix-network`: `AdminCommand { op, args, sig }` / `AdminReply`.
- Emparejamiento: el teléfono admin genera su llave; se empareja al ancla vía QR en primer setup; el ancla guarda la llave del teléfono como "controlador autorizado".
- El ancla valida firma del controlador y ejecuta: alta/baja de dispositivos, cambios de rol, revocación (`block_peer`), emitir roster.
- La llave admin raíz nunca se transfiere al teléfono.

### BM3 — App unificada role-aware (merge admin UI → client)
- Fusionar las pantallas de gobierno del admin (device mgmt, roles, audit, schema explorer, SQL console) en la app cliente como **módulo "Gobierno"** visible solo si el rol es admin (alinea con `vision.md:62` "🔧 Gobierno de la Matriz").
- Cuando el usuario es admin, esas pantallas operan como **control remoto del ancla** (BM2), no con autoridad local.
- Responsive: las pantallas admin deben funcionar en móvil (bottom sheets / vistas compactas).
- Deprecar `apps/admin` como app standalone una vez migrada la UI (mantener su backend como base del ancla headless).

### BM4 — Replicación scoped (escala móvil)
- Definir el alcance de replicación por rol/permisos (reusar `can_open`/`can_write` por entidad + row-level de la visión).
- Catch-up scoped: el ancla responde solo el slice permitido (extender `snapshot_org_rows` con filtro por rol/entidad/rango).
- Query on-demand: comando para pedir al ancla filas fuera del slice local (paginado).
- Evitar replicar 1M+ filas al teléfono; medir tamaño de réplica típica.

### BM5 — Modelo foreground-sync + batería/datos
- Sync agresivo al abrir/volver a foreground (catch-up scoped + drenar CDC).
- Throttling: bajar frecuencia de heartbeats en móvil, pausar loops en background, respetar Doze/suspensión.
- Snapshots (liga con plan de escala) para que el sync de apertura sea rápido.

### BM6 — Push opt-in (conveniencia, no requisito)
- Integrar APNs/FCM como wake-to-sync opt-in. Requiere un **gateway de push de Syntrix** (dependencia central, documentarla como no-soberana).
- El ancla notifica al gateway "hay cambios para el dispositivo X"; el gateway emite push; el móvil despierta y sincroniza en su ventana permitida.

### BM7 — Trait de abstracción de transporte + spike Zenoh (largo plazo)
- Definir trait `Transport` (publish/subscribe/query/discover/connect) en `syntrix-network`; libp2p como impl default.
- Spike Zenoh detrás del trait: ancla = router Zenoh; móviles = clientes; key expressions `org/{id}/{entity}/**`; queryables para replicación scoped; storage sobre Turso.
- Criterio de decisión: comparar contra libp2p bajo carga de orgs grandes + replicación scoped antes de comprometer v2.

## B.5 Riesgos (Parte B)

| Riesgo | Mitigación |
|---|---|
| El pitch "sin servidores" choca con el ancla obligatoria | Reencuadrar: "sin *nuestros* servidores obligatorios; el nodo siempre-activo es tuyo". Ofrecer ancla self-host y ancla hosteada. |
| Ancla detrás de NAT de oficina sin endpoint público | Relay (Parte A) + preferir VPS/port-forward; documentar setup. |
| Merge admin→client es refactor grande | Fasear: BM3 puede ir después del lanzamiento con app cliente móvil + admin desktop como control interino del ancla. |
| Llave admin en el ancla (VPS) = superficie de ataque | Cifrado en reposo del `--keypair-file`; ancla en hardware del cliente cuando la soberanía sea crítica. |
| Push reintroduce dependencia central | Opt-in, claramente etiquetado; el sistema funciona sin él (foreground-sync). |
| Replicación scoped mal definida filtra datos entre roles | Reusar y testear el modelo de permisos por autor/rol ya existente; tests de aislamiento. |
| Rewrite prematuro a Zenoh | Solo detrás del trait y tras spike con datos reales; libp2p es el default de lanzamiento. |

## B.6 Validación (Parte B)

1. App unificada compila y arranca en Android e iOS (paths de sandbox correctos, Limbo escribe).
2. Empleado en móvil (4G/CGNAT) conecta al ancla vía endpoint público y sincroniza su slice scoped en foreground.
3. Admin en móvil empareja con el ancla (QR) y ejecuta alta/baja/revocación vía `/syntrix/admin-control/1` sin poseer la llave raíz.
4. Ancla sobrevive a que todos los móviles estén dormidos; un móvil que despierta hace catch-up correcto.
5. Aislamiento: un rol no recibe en su réplica scoped datos fuera de sus permisos.
6. (v2, si se prototipa) Zenoh detrás del trait pasa los mismos tests de sync/aislamiento que libp2p.

## B.7 Decisiones abiertas (para confirmar en implementación)

- ¿El ancla hosteada por Syntrix es parte del modelo de negocio (SaaS) además del self-host? (afecta pricing y el reencuadre del pitch).
- ¿BM3 (merge admin→client) va antes o después del primer lanzamiento móvil? (recomendado: después; lanzar con cliente móvil + admin desktop interino).
- ¿Se soporta más de un ancla por org (alta disponibilidad / réplica) o una sola al inicio? (recomendado: una al inicio, HA después).
