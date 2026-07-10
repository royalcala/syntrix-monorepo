# ADR: Framework de red — rust-libp2p vs Eclipse Zenoh

> **Estado**: Aceptado — mantener `rust-libp2p`. Zenoh reservado para un posible pivote a edge/industrial (ver disparadores).
> **Fecha**: 2026-07-10.
> **Ámbito**: capa de red (`crates/syntrix-network`) — pub/sub, discovery, catch-up y transporte.
> **Origen**: evaluación de una comparativa externa que propone Zenoh como competidor/complemento de libp2p.
> **Relacionado**: `1783695719135-adr-libp2p-vs-headscale.md`, `1783470696465-adr-libp2p-vs-iroh-litep2p.md` (mismo patrón: recomendación externa de cambiar la capa de red).

## Contexto

Syntrix es un ERP descentralizado P2P, offline-first: "Sin servidores. Sin VPN." (`README.md:3`),
"Soberanía Absoluta — nadie te puede desconectar" (`identidad.md:27`). Perfil real: **3–10 peers
por org**, multi-org con aislamiento por topic, desktop-only (Tauri v2), NAT traversal necesario
para redes móviles/CGNAT/oficina. El sync está **desacoplado del transporte** (vive en Turso/Limbo
+ CDC + LWW por `change_time`), decisión deliberada tras el fracaso del sync acoplado de Iroh
(`1782758526139`: *"fork is fragile, sync is unreliable"*).

**Eclipse Zenoh** (ZettaScale, escrito en Rust) es un protocolo pub/sub + query + storage que
unifica datos en movimiento, en reposo y computación bajo *key expressions* jerárquicas. Se usa
en robótica (RMW alternativo de ROS 2), automotriz e industrial.

## Decisión

**Mantener `rust-libp2p`.** No adoptar Zenoh como capa de red. **No es un complemento viable** de
libp2p (compite en la misma capa: mensajería/discovery/transporte). Se reserva como alternativa
*seleccionable por-deployment* solo si el producto pivotea a edge/industrial (ver disparadores).

## A diferencia de Headscale, Zenoh SÍ es competidor de misma capa

```
┌──────────────────────────────────────────────────────────────┐
│ APP     │ CDC · LWW · permisos por rol · invite · catchup      │ ← Turso + syntrix-core (queda)
├─────────┼──────────────────────────────────────────────────────┤
│ MENSAJE │ pub/sub · query        ◀── Zenoh reemplaza gossipsub+rr │ ← syntrix-network
│ DISCOVER│ quién-es-quién por org ◀── Zenoh scouting reemplaza Kad │ ← syntrix-network
├─────────┼──────────────────────────────────────────────────────┤
│ TRANSP. │ NAT traversal · relay  ◀── AQUÍ Zenoh es MÁS DÉBIL      │ ← syntrix-network
└─────────┴──────────────────────────────────────────────────────┘
```

Zenoh podría reemplazar 3 de las 4 franjas (gossipsub, request_response, Kademlia y transporte).
Headscale solo cubría la de abajo (L3). Por eso Zenoh se evalúa como reemplazo serio, no como
add-on: correr ambos a la vez duplica conexiones, discovery y transporte.

## Comparativa por criterio del use case

| Criterio | rust-libp2p (actual) | Eclipse Zenoh |
|---|---|---|
| Naturaleza | Framework P2P (transporte+msg+discovery) | Protocolo pub/sub + query + storage |
| Servidor central obligatorio | **No** (relay opcional, stateless, E2E) | **No en LAN**; **sí router en data-path** para cruzar NAT |
| **NAT traversal (punto decisivo)** | autonat + **dcutr (hole punch)** + circuit relay fallback E2E | **Sin hole punching**; cruzar NAT ⇒ **router público que reenvía los datos** |
| Discovery LAN | Requiere habilitar `mdns` (gap actual) | **Multicast scouting nativo** (resolvería el gap) |
| Discovery cross-red | Kademlia providers | Gossip scouting vía routers (necesita router) |
| Pub/sub | gossipsub (mesh + gossip) | Publishers/subscribers (menos overhead) |
| Catch-up / query histórica | `request_response` hand-rolled | **Queryables + storages** (query distribuida nativa) — más elegante |
| Aislamiento multi-org / entidad | 1 topic gossipsub por org | **Key expressions** `org/{id}/{entity}/**` — mapea perfecto a namespaces |
| Permisos | Por evento/rol/autor (app-level) + `block_peer` | **ACL por key expression** (config) + firma de autor app-level |
| Identidad | Ed25519 PeerID = autor de fila CDC | ZID de sesión (no cripto-identidad de autor) |
| Sync semantics | Desacoplado (Turso/CDC/LWW) | Riesgo: storages+replication **re-acoplan** el sync |
| Transporte | QUIC + TCP | QUIC + TCP + TLS + WS + **SHM zero-copy** |
| Rendimiento | Bueno (Swarm polling, HoL a escala) | **Excelente** (diseñado para latencia/throughput extremos) |
| Escala CPU por nodo | Swarm de 1 task (HoL en nodo hub / org grande) | Mejor throughput; pero beneficio atado a router para NAT |
| Madurez | Alta (IPFS/Ethereum/Filecoin) | Alta en robótica/IoT/edge (ROS 2, industrial) |
| Rust-native / Tauri | ✅ crate nativo | ✅ crate nativo |
| Trabajo ya hecho | Migración libp2p + NAT committed | Migrar = reescribir msg+discovery+catch-up |
| Fit con los 4 pilares | ✅ sin servidor / hole-punch directo | ⚠️ cross-NAT necesita router en data-path |

## El punto decisivo: NAT traversal

Es lo que define la decisión para el mercado real de Syntrix (nodos en redes caseras/móviles):

- **libp2p**: intenta conexión **directa** (dcutr/hole punching). Solo si falla, usa relay como
  fallback, reenviando tráfico **cifrado E2E** (el relay no lee nada). Meta: siempre P2P directo.
- **Zenoh**: **no hace hole punching**. Dos peers detrás de NAT que no se ven necesitan un
  **router público común** que **enruta los datos a nivel de protocolo** (está en el data-path).

Adoptar Zenoh para "fuera de LAN" **reintroduce un servidor en el camino de los datos** — lo que
el pitch se vende como no tener. Es análogo al problema de Headscale (que metía servidor en el
*control plane*), pero de hecho peor para el pitch: aquí el servidor está en el *data plane*.

## El segundo riesgo: re-acoplar el sync (repetir el error de Iroh)

El mayor atractivo de Zenoh son sus **storages + replication** (darían catch-up e historia
"gratis"). Pero eso **re-acopla el sync al transporte** — exactamente el patrón que Syntrix ya
descartó al abandonar `iroh-docs`/`iroh-blobs` por frágil. La única vía sana sería usar Zenoh
**solo como transporte pub/sub + query** manteniendo Turso/CDC — lo que reduce buena parte de su
ventaja.

## Dónde Zenoh sería genuinamente mejor (siendo justos)

- **Key expressions** mapean a namespace-por-entidad mejor que "un topic por org", con ACL por
  clave granular.
- **Query distribuida nativa** (`get`/queryable) es un primitivo más limpio que el
  `request_response` hand-rolled para catch-up.
- **Scouting multicast** resuelve el gap de discovery LAN out-of-the-box.
- **Rendimiento** superior y **SHM zero-copy** (útil intra-host: admin+client en la misma máquina).

## Consecuencias

- **Positivo**: se preserva la migración libp2p + trabajo NAT; NAT traversal directo (sin servidor
  en data-path); sync desacoplado intacto; identidad = autorización; coherente con los 4 pilares.
- **Negativo / lo que se cede**: se renuncia al modelo de key expressions, query nativa y
  rendimiento de Zenoh. Se neutraliza cerrando los gaps de conectividad de libp2p
  (`1783695719136-p2p-connectivity-hardening-plan.md`) y modelando topics estilo `org/{id}/{entity}`
  sobre gossipsub sin adoptar el runtime de Zenoh.
- **No es complemento**: correr Zenoh junto a libp2p duplica capas; se elige uno u otro.

## Revisión

Reconsiderar (con más fuerza que Headscale) si se cumple alguno de estos disparadores:

- El producto pivotea a **edge/IoT/robótica/industrial** en LAN o intranets controladas (donde hay
  routers y el NAT no es hostil): Zenoh sería superior a libp2p ahí → evaluar vía trait de
  transporte con backend Zenoh seleccionable por-deployment.
- El rendimiento/latencia de la mensajería se vuelve cuello de botella **y** se acepta desplegar
  routers de infraestructura como parte del modelo.

## Nota: escala y complementos reales (fuera del alcance de este ADR)

Para escalar, el camino alineado con la arquitectura actual **no es Zenoh** sino: **litep2p**
(drop-in, protocolo-compatible, cuando el Swarm de un nodo hub / org grande sea cuello de CPU),
**snapshots + particionamiento por namespace** (capa de datos), más **relays self-hosted**
adicionales, y **`iroh-blobs` como librería** para transferencia de blobs grandes (adjuntos, pesos
de modelos IA). Ese sí es complemento genuino (otra capa), a diferencia de Zenoh.
