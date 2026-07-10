# ADR: Framework de red — rust-libp2p vs Headscale/Tailscale (WireGuard)

> **Estado**: Aceptado — mantener `rust-libp2p`. Headscale/Tailscale queda como *underlay* opcional de deployment, no como arquitectura.
> **Fecha**: 2026-07-10.
> **Ámbito**: capa de red (`crates/syntrix-network`, `apps/syntrix-relay`, `identity.rs` de client/admin).
> **Origen**: evaluación de una comparativa externa (Gemini) que recomendaba reemplazar libp2p por Headscale.
> **Relacionado**: `1783470696465-adr-libp2p-vs-iroh-litep2p.md` (mismo patrón: recomendación externa de migrar la capa de red, rechazada). Follow-ups accionables en `1783695719136-p2p-connectivity-hardening-plan.md`.

## Contexto

Syntrix es un ERP descentralizado P2P, offline-first, cuyo pitch e identidad son explícitos:
"Sin servidores. Sin VPN." (`README.md:3`) y "Soberanía Absoluta — nadie te puede desconectar"
(`identidad.md:27`). Perfil real: **3–10 peers por org**, multi-org con aislamiento por topic
gossipsub, desktop-only (Tauri v2), NAT traversal necesario para redes móviles/CGNAT/oficina.

Gemini propone reemplazar "toda la implementación libp2p" por **Headscale** (reimplementación
open-source del control server de Tailscale) + clientes Tailscale (WireGuard).

## Decisión

**Mantener `rust-libp2p` como capa de red.** No adoptar Headscale como arquitectura.
**Permitir Tailscale/Headscale como *underlay* opcional** en despliegues enterprise donde el
cliente ya opera su propia tailnet (Syntrix corre sobre esas IPs y salta el NAT traversal).

## Análisis: no están en la misma capa

El error de encuadre de Gemini es comparar una VPN L3 (solo conectividad) contra un stack P2P
completo. Headscale reemplazaría ~25% de `syntrix-network` (transporte + NAT) y **0%** de la
capa de mensajería/discovery/app, que es el grueso del trabajo:

```
                 Lo que Syntrix necesita
┌──────────────────────────────────────────────────────────────┐
│ APP     │ CDC · LWW · permisos por rol · invite · catchup      │ ← Turso + syntrix-core
├─────────┼──────────────────────────────────────────────────────┤
│ MENSAJE │ pub/sub fan-out (gossipsub) · request_response       │ ← syntrix-network
│ DISCOVER│ quién-es-quién por org (Kademlia providers)          │ ← syntrix-network
├─────────┼──────────────────────────────────────────────────────┤
│ TRANSP. │ NAT traversal · cifrado · relay        ◀── AQUÍ Headscale │ ← syntrix-network
└─────────┴──────────────────────────────────────────────────────┘
```

Con Headscale obtienes una red de IPs (100.x.y.z) y **tendrías que reconstruir encima** el bus
de eventos, discovery por org, invite, catch-up y la validación por autor — probablemente
corriendo gossipsub sobre las IPs de Tailscale (doble cifrado, trabajo duplicado).

## ¿Qué es DERP? (aclaración pedida)

**DERP** = *Designated Encrypted Relay for Packets*, el sistema de relays de Tailscale. Reenvía
paquetes WireGuard **ya cifrados** (no puede leerlos), corre sobre HTTPS/443 (atraviesa
firewalls), y hace de canal de señalización para el hole punching. Toda conexión arranca por
DERP y se "promueve" a directa cuando el hole punch funciona. Es auto-hospedable.

**Equivalencia en libp2p (ya presente en el código):** DERP ≈ Circuit Relay v2
(`apps/syntrix-relay`) + dcutr (`behaviour.rs:16`) + identify/autonat (señalización) + bootstrap
nodes. La diferencia no es *capacidad*, es *madurez y tasa de éxito en campo*.

## Comparativa por criterio del use case

| Criterio | rust-libp2p (actual) | Headscale + Tailscale/WireGuard |
|---|---|---|
| Naturaleza | Framework P2P completo | Overlay VPN L3 (solo IPs) |
| Servidor central obligatorio | **No** (relay opcional, stateless) | **Sí** (control plane para enrolar/renovar/revocar) |
| NAT traversal (calidad) | autonat+dcutr+relay, menos probado | **Superior** (DERP + hole punch, battle-tested) |
| UDP bloqueado | TCP fallback (`lib.rs:127`) | DERP sobre 443 |
| LAN sin internet | Requiere habilitar `mdns` (gap) | Descubrimiento LAN nativo |
| ¿Datos por el server? | No (relay solo fallback, cifrado) | No (DERP solo fallback, cifrado) — empate |
| Identidad | Ed25519 PeerID auto-soberano = autor de fila CDC | Node key gestionada por control plane; ≠ identidad de autor |
| Autorización / revocación | Por evento/rol/autor + `block_peer`, sin server | ACL de red (IP/puerto), grueso, depende del server online |
| Discovery por org | Kademlia providers `hash("syntrix-p2p-"+org)` | No existe; mapear org→tailnet/ACL a mano |
| Pub/sub fan-out | gossipsub | No lo da |
| Catch-up / invite | `request_response` nativo | No lo da |
| Aislamiento multi-org | 1 topic gossipsub por org (limpio) | 1 tailnet/ACL por org (grueso) |
| Expiración de llaves | No expiran (persistidas en disco) | Node keys expiran (180d por defecto) → re-auth |
| Onboarding | Invite P2P por ticket (`send_invite`) | Pre-auth keys / aprobación en server central |
| Integración en app | Crate Rust nativo (Tauri) | `tsnet` (Go) incómodo en Rust, o demonio Tailscale instalado |
| Carga operativa | `syntrix-relay` opcional (stateless) | Headscale (DB/TLS/HA/backups) + DERP propios |
| Escala CPU | Swarm por polling (HoL a cientos; irrelevante a 3–10) | WireGuard kernel, escala a miles |
| Madurez | Alta (IPFS/Ethereum/Filecoin) | Altísima |
| Trabajo ya hecho | Migración Iroh→libp2p + NAT/relay committed | Migrar = tirarlo y reconstruir |
| Fit con los 4 pilares | ✅ sin servidor/sin VPN | ❌ control plane central contradice pilar #1 y #4 |

## Escenarios de supervivencia (los 4 de Gemini) re-evaluados

| Escenario | libp2p | Headscale | Mejor para Syntrix |
|---|---|---|---|
| LAN sin internet | ✅ con `mdns` (falta) | ✅ nativo | Headscale hoy; empate tras habilitar `mdns` |
| NAT estricto / relay | ✅ (menos probado) | ✅ (más probado) | **Headscale** (tasa de éxito) |
| Apocalipsis (server muere) | ✅ se auto-sana, no hay server | ⚠️ sobrevive lo conectado, no enrola/revoca | **libp2p** |
| Soberanía / sin dependencia | ✅ pilar del producto | ❌ control plane central | **libp2p** |

## Correcciones a la comparativa externa (Gemini)

- **"Mejor que toda la implementación libp2p"**: falso de raíz — compara VPN L3 contra stack P2P
  completo. Solo reemplaza el transporte.
- **"El servidor central no es un embudo"**: cierto para *datos*, pero **sí es embudo para el
  plano de control** (enrolamiento, revocación, re-NAT en frío). libp2p no tiene ese embudo.
- **"libp2p usa mDNS en LAN"**: es capacidad de libp2p, pero **no está habilitada** en Syntrix
  (`syntrix-network/Cargo.toml:8`). Hoy la LAN pura depende de dial directo/Kademlia.
- **"Relés ultraligeros solo con Tailscale"**: `apps/syntrix-relay` ya es eso y es *opcional*,
  mientras Headscale añade un control plane *no opcional*.
- **"libp2p se auto-sana mejor en el apocalipsis"**: correcto, y es precisamente el diferenciador
  de Syntrix (no hay directorio que firmar).

## Consecuencias

- **Positivo**: se preserva la migración libp2p + trabajo NAT; identidad = autorización sin
  servidor; stack completo (pub/sub + discovery + req/resp + transporte) en un solo crate;
  coherente con los 4 pilares.
- **Negativo / gap único**: el NAT traversal de libp2p es menos turnkey que DERP. Se neutraliza
  cerrando los gaps de conectividad (ver plan de hardening).
- **Opción abierta**: Tailscale/Headscale como *underlay* de deployment (correr Syntrix sobre la
  tailnet del cliente), sin cambiar la arquitectura ni introducir dependencia obligatoria.

## Revisión

Reconsiderar si se cumple alguno de estos disparadores:

- La tasa de conexión directa P2P se mantiene inaceptablemente baja **tras** cerrar los gaps del
  plan de hardening (relay reservation + bootstrap + mDNS + dcutr real).
- El producto pivotea y acepta un control plane central obligatorio como requisito de negocio
  (contradiría los pilares actuales).
