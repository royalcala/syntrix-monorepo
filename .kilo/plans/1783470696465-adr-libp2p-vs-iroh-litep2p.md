# ADR: Framework P2P — rust-libp2p vs Iroh vs litep2p

> **Estado**: Aceptado — mantener `rust-libp2p`.
> **Fecha**: 2026-07-07.
> **Ámbito**: capa de red (`crates/syntrix-network`) del monorepo Syntrix P2P.
> **Origen**: evaluación de una comparativa externa (Gemini) que recomendaba migrar a Iroh.

## Contexto

Syntrix es un ERP descentralizado P2P, offline-first, sin servidores ni VPN.

Perfil de uso real:

- **3–10 peers por org** (ver `plan-maestro.md:128`, riesgo en `1783371801395-libp2p-nat-reconnect-plan.md:342`).
- Multi-org con **aislamiento por topic** (un topic gossipsub por org).
- **Event bus** vía gossipsub + **discovery** vía Kademlia providers.
- **invite/catchup** vía `request_response` nativo de libp2p.
- **Sync desacoplado del transporte**: vive en Turso/Limbo (SQL) + CDC con LWW + HLC.
- Desktop-only (Tauri v2). Necesita NAT traversal para redes móviles/CGNAT/oficina.

## Historial de decisiones (por qué NO es una elección nueva)

1. **Inicio**: Iroh (`iroh` + fork `royalcala/iroh-docs` + `iroh-blobs` + redb).
2. **Se quitó `iroh-docs`/`iroh-blobs`** — razón textual: *"fork is fragile, sync is unreliable"* (`1782758526139-remove-iroh-docs-gossip-sync.md:45`).
3. **Migración completa Iroh → rust-libp2p** (`1782854204807-iroh-to-libp2p-migration.md`, ✅ completa): gossipsub, Kademlia, request_response, QUIC + TCP fallback, encapsulado en `syntrix-network`.
4. **Hoy**: libp2p 0.56 con `autonat` + `relay` + `dcutr`, `block_peer`, auto-reconnect con backoff (2s→64s) y peer scoring — committed (`d6b2c7d`). Sin rastro de iroh en ningún `Cargo.toml`.

## Decisión

**Mantener `rust-libp2p`. No migrar a Iroh ni a litep2p.**

## Comparativa por criterio del *use case*

| Criterio (según nuestro uso) | rust-libp2p (actual) | Iroh | litep2p |
|---|---|---|---|
| Pub/sub por org (event bus) | Gossipsub first-class | `iroh-gossip` fuera del core, mantenimiento comunitario | Gossipsub compatible |
| Discovery entre peers de una org | Kademlia providers (ya usado) | Punto-a-punto: requiere NodeId + relay conocidos, sin DHT | Kademlia compatible |
| Multi-transporte (firewalls que bloquean UDP) | QUIC + **TCP fallback** | QUIC/UDP puro (depende de relay si UDP bloqueado) | QUIC + TCP |
| Acoplamiento con sync | Solo transporte (sync en Turso/CDC) | "Baterías incluidas" (docs/blobs) — lo que ya nos falló | Solo transporte |
| NAT traversal | autonat + dcutr + relay client (ya integrados) | Relay DERP **turnkey y encendido por defecto** | Tooling de traversal más pobre (nodos públicos) |
| CPU a escala | Swarm por polling (HoL blocking bajo carga masiva) | QUIC nativo, sin Swarm central | Task por conexión, mejor multinúcleo |
| Gobernanza / vendor | Consorcio amplio (IPFS/Ethereum/Filecoin) | Startup única (n0), historial de mover crates fuera del core | Parity (Polkadot/Substrate) |
| Madurez de ecosistema | Alta (docs, ejemplos) | Media | Joven, mayormente interno de Parity |

### Correcciones a la comparativa externa (Gemini)

- **"~99% Iroh vs ~70% libp2p"** es peras con manzanas: el 99% de Iroh cuenta el *fallback a relay* como éxito. libp2p con relay siempre-activo + dcutr también llega a ~100% de conectividad relayed. La ventaja real de Iroh no es la tasa, es que el relay viene turnkey.
- **"litep2p ~70% NAT directo"** es generoso: litep2p nació para validadores Polkadot (nodos públicos); su tooling de traversal es *menor* que el de rust-libp2p.
- **"relés ultraligeros solo con Iroh/NixOS"** es falso como diferenciador: el circuit-relay-v2 de libp2p también es self-hosteable — de hecho ya está planeado como `syntrix-relay` (`1783371801395-...:328`).
- **"sync nativo de Iroh" como ventaja estrella** es precisamente lo que arrancamos por frágil; hoy el sync desacoplado (Turso/CDC) es arquitectónicamente más sano.

## Consecuencias

- **Positivo**: se preserva la migración libp2p + trabajo NAT ya terminado; sync independiente del vendor; un solo paquete maduro con pub/sub + DHT + multi-transporte + traversal.
- **Negativo / gap único**: el relay de libp2p no viene "encendido" como el DERP de Iroh. Se neutraliza self-hosteando `syntrix-relay`.
- **Reservas para el futuro** (no ahora):
  - **litep2p** solo si el `Swarm` se vuelve cuello de CPU *a escala* (cientos de peers o transferencias masivas). A 3–10 peers/org es optimización prematura. Es drop-in compatible con protocolos libp2p.
  - **`iroh-blobs` como librería** (BLAKE3, streaming verificado, content-addressed) *si* el transfer de pesos/embeddings se vuelve central — junto a libp2p, sin adoptar Iroh como plano de control.

## Seguimiento (fuera del alcance de este ADR)

- Cerrar el **relay self-hosted** (`syntrix-relay` + `with_relay_client`) — es el pendiente P1 del roadmap (`1783367768611-roadmap-tests-nat-inference.md:43`) y lo único que cierra el gap real frente a Iroh.

## Revisión

Reconsiderar esta decisión si se cumple alguno de estos disparadores:

- Peers por org supera de forma sostenida el orden de las decenas altas / cientos, con CPU del nodo saturado por el Swarm → evaluar **litep2p**.
- El transfer de blobs grandes (pesos de modelos) se vuelve caso de uso central → evaluar **`iroh-blobs` como librería**.
