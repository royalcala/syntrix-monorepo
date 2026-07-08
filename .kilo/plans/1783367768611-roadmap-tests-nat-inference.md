# Roadmap: Fase A Test Hardening → libp2p NAT → Capability-Tiered Inference

> **Estado**: Handover — 2026-07-07. Todo el código está committed en `well-sink`.
> **Sesión actual**: https://kilo.syntrix.ai/sessions/1783367768611

## ✅ Completado esta sesión

### Fase 0 — Endurecer Fase A
| Item | Estado |
|------|--------|
| 0.1 Build verde | ✅ reqwest fix, migrations split con statement-breakpoint, FK workaround en storage.rs |
| 0.2 Tests Rust (view_defs, ia_queries, device_type) | ✅ 7 tests en `ai_views_test.rs` |
| 0.2 device_type | ✅ Test en `sync_test.rs` + `binary_e2e_test.rs` |
| 0.3 Binary E2E (ai_status, ai_chat, device_type) | ✅ 3 nuevas tests en `binary_e2e_test.rs` |
| 0.4 Frontend tests | ✅ 46 tests (catalog, HomeScreen, ViewScreen, VoiceInput, admin pages) |
| 0.5 Docs | ✅ AGENTS.md refrescado, `tool-call-parser.md` SUPERSEDED |

### Fase 1 — libp2p NAT (núcleo)
| Item | Cambios |
|------|---------|
| TCP fallback (`/tcp/0`) | `identity.rs` (admin+client) |
| `block_peer()` | `lib.rs`, `admin.rs` |
| Auto-reconnect + backoff 2s→64s | `lib.rs` reconnect_loop, `identity.rs` PeerDisconnected handler |
| Peer scoring | `lib.rs` peer_scores tracking |
| Kademlia providers (`discover_org_peers`) | `lib.rs`, `identity.rs` |
| Bootstrap nodes IPFS | `lib.rs` P2PNode::new |
| Re-announce on NewListenAddr | `lib.rs` handle_swarm_event |
| Test block_peer.rs | `crates/syntrix-network/tests/block_peer.rs` |

### Fase 2 — Capability-Tiered Inference (sin spike)
| Item | Archivo |
|------|---------|
| `DeviceTier` enum + `from_specs()` | `crates/syntrix-ai/src/router.rs` |
| `TaskTier` enum + mapping | `router.rs` |
| `CascadeDecision::resolve()` | `router.rs` |
| `strip_think_tags()` | `router.rs` |
| 16 unit tests | `router.rs::mod tests` |

## ⬜ Pendiente (priorizado)

| # | Item | Bloqueante | Dificultad | Propuesta |
|---|------|-----------|------------|-----------|
| P1 | **libp2p upgrade 0.54→0.56+** (relay transport) | Ninguno | Media | Migrar SwarmBuilder a phased API. `with_relay_client` existe en 0.56+. |
| P2 | **CDC-sync view_defs binary e2e** | Schema registry | Baja | Registrar `view_definitions`/`ia_queries` en schema para CDC |
| P3 | **Fase 2: Pipeline Manager** (PeerDelegateBackend) | Spike llama-cpp | Alta | No iniciar sin spike |
| P4 | **docs/apps/docs/** estado-actual | Ninguno | Baja | Actualizar con hallazgo benchmark 0% |
| P5 | **useIAQueue test** | Ninguno | Baja | Test del hook |

## Archivos clave creados/modificados

```
apps/admin/src-tauri/src/admin.rs          # +block_peer en deactivate
apps/admin/src-tauri/src/identity.rs       # +TCP listen, +reconnect handler
apps/admin/src-tauri/tests/binary_e2e_test.rs  # +3 tests (ai_status, ai_chat, device_type)
apps/client/src-tauri/src/identity.rs      # +TCP listen, +reconnect handler
apps/client/src-tauri/src/lib.rs           # +drizzle_execute headless
apps/client/src-tauri/tests/ai_views_test.rs   # NUEVO: 7 tests
apps/client/src-tauri/tests/common/mod.rs  # NUEVO: helpers
apps/client/src/__tests__/components/catalog/*.test.tsx  # 5 archivos, 23 tests
apps/client/src/__tests__/screens/HomeScreen.test.tsx    # NUEVO
apps/client/src/__tests__/screens/ViewScreen.test.tsx    # NUEVO
apps/client/src/__tests__/components/VoiceInput.test.tsx # NUEVO
apps/admin/src/__tests__/screens/ViewsPage.test.tsx      # NUEVO
apps/admin/src/__tests__/screens/ModulesPage.test.tsx    # NUEVO
crates/syntrix-ai/src/router.rs          # +DeviceTier, TaskTier, CascadeDecision, strip_think_tags
crates/syntrix-network/src/behaviour.rs   # +autonat, relay::client, dcutr behaviours
crates/syntrix-network/src/lib.rs         # +block_peer, reconnect, scoring, bootstrap, providers
crates/syntrix-network/tests/block_peer.rs # NUEVO
packages/shared-drizzle/src/entities.ts   # FK inválidas removidas
.ai/AGENTS.md                             # Refrescado completo
.kilo/plans/1783125232147-tool-call-parser.md  # Marcado SUPERSEDED
justfile                                  # Nota test-binary-e2e
```

## Notas técnicas

- **libp2p upgrade**: Nuestro `Cargo.toml` usa `version = "0.54"`. crates.io tiene `0.56.0`. El fork local está en `0.57.0-dev`. Subir a `"0.56"` da acceso a `with_relay_client` pero requiere migrar SwarmBuilder a la API phased (cambia `behaviour.rs` y `lib.rs`).
- **relay transport**: No necesario para LAN/localhost. Requerido para NAT traversal cross-red.
- **Tests que requieren GTK local**: `just test-binary-e2e` (compila remoto, ejecuta local).
- **Test count final**: Rust: 51 tests (syntrix-ai) + 7 (ai_views_test) + existing. Frontend: 46 (client) + 26 (admin).
