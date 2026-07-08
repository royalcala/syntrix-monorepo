# Handover — Camino para la próxima sesión

> **Creado**: 2026-07-07 18:57. Rama: `well-sink`. Todo committed (último: `0e1fe76`).
> Este documento consolida lo terminado, lo pendiente, y los planes nuevos que aparecieron,
> para arrancar mañana sin re-descubrir el estado.

---

## 1. Lo que se TERMINÓ esta sesión

### Fase 0 — Endurecer Fase A (test hardening) ✅ COMPLETA
| Item | Detalle | Archivos |
|------|---------|----------|
| Build verde | reqwest a deps regulares, split de migraciones por `--> statement-breakpoint`, FK workaround | `crates/syntrix-ai/Cargo.toml`, `apps/*/src-tauri/src/storage.rs` |
| Rust Tier 1/2 | 7 tests view_definitions/ia_queries CRUD + LWW | `apps/client/src-tauri/tests/ai_views_test.rs`, `tests/common/mod.rs` |
| device_type | default "client" + update a "client-ia" | `apps/admin/src-tauri/tests/sync_test.rs` |
| Binary E2E | ai_status, ai_chat_degraded, device_type_propagation | `apps/admin/src-tauri/tests/binary_e2e_test.rs` |
| Frontend | 75 tests (catalog×23, screens, admin pages, useIAQueue×3) | `apps/*/src/__tests__/**` |
| Docs | AGENTS.md refrescado + regla NO-EDITAR-SQL, estado-actual con benchmark 0% | `.ai/AGENTS.md`, `apps/docs/src/content/docs/estado-actual.md` |

### Fase 1 — libp2p NAT (plan `1783371801395`) ✅ ~90%
| Item | Estado |
|------|--------|
| libp2p 0.54→0.56 | ✅ `connection_id` fields, builder compatible |
| relay transport (`with_relay_client`) | ✅ noise+yamux combinado con QUIC+DNS |
| autonat + relay::client + dcutr | ✅ behaviours en `behaviour.rs` |
| TCP fallback `/tcp/0` | ✅ ambos apps |
| block_peer + reconnect backoff 2s→64s | ✅ `lib.rs`, `admin.rs`, `identity.rs` |
| peer scoring, Kademlia providers, bootstrap IPFS, re-announce | ✅ `lib.rs` |
| Test block_peer.rs | ✅ |

### Fase 2 — Capability-Tiered Inference (plan `1783452736636`) ✅ T2-T7 sin backend
| Item | Estado |
|------|--------|
| DeviceTier + from_specs() | ✅ `crates/syntrix-ai/src/router.rs` |
| TaskTier + mapping | ✅ |
| CascadeDecision::resolve() | ✅ |
| strip_think_tags() | ✅ |
| 16 unit tests (51 total en syntrix-ai) | ✅ |

**Resultado de tests**: Rust 58 (syntrix-ai 51 + ai_views 7) · Frontend 75 (client 49 + admin 26).

---

## 2. Lo que quedó PENDIENTE

| # | Item | Plan origen | Bloqueante | Dificultad |
|---|------|-------------|------------|------------|
| P1 | **Tests relay/reconnect** (T9) — `reconnect_basic.rs`, `relay_basic.rs` | `1783371801395` | Ninguno | Baja |
| P2 | **Binario `syntrix-relay`** self-hosted (T10) — cierra el gap NAT vs Iroh del ADR | `1783371801395` + `1783470696465` | Ninguno | Media |
| P3 | **Spike llama-cpp-2 cross-compile** (Nix + Android NDK + iOS) — **BLOQUEA toda Fase 2 real** | `1783452736636` T1 | — | Alta |
| P4 | **InferenceBackend + LlamaCppBackend + probe + GBNF + PeerDelegate + weights P2P** | `1783452736636` T2-T12 | P3 spike | Alta |
| P5 | **Refactor migraciones + runner automático** (`syntrix-migrate`, squash a 0000) | `1783465859144` | Ninguno | Media |
| P6 | **Pruning `turso_cdc`** (watermark por org activo) | `1783477000000` | debajo de P5 | Baja |

**Nota CDC-sync view_defs**: descartado — `view_definitions`/`ia_queries` son tablas locales, no de negocio; no van por CDC (no están en el schema registry a propósito).

---

## 3. Planes NUEVOS que aparecieron durante la sesión

Fueron creados por otros procesos/agentes mientras trabajaba. Los reviso aquí para que mañana no sorprendan:

| Plan | Qué propone | Prioridad sugerida |
|------|-------------|--------------------|
| **`1783465859144-db-migrations-drizzle-refactor.md`** | Runner de migraciones robusto (`syntrix-migrate` con `__migrations` table + `_journal.json`), squash a `0000` limpio, quitar ORM runtime de Drizzle (solo schema source), comando SQL genérico tipado, helper de escrituras que estampa `change_time`/`node_id`. **Toca correctness** (bug de no-replicación por `node_id` vacío). | **Alta** — es el que arregla deuda real de la capa DB |
| **`1783470696465-adr-libp2p-vs-iroh-litep2p.md`** | ADR (decisión aceptada): **mantener rust-libp2p**, no migrar a Iroh/litep2p. Único gap real = relay self-hosted (= mi P2). Reserva litep2p para escala y iroh-blobs para transfer de weights. | Referencia — no requiere acción, solo respalda P2 |
| **`1783477000000-cdc-turso-pruning.md`** | Poda de `turso_cdc` (`DELETE WHERE change_id <= watermark`, watermark = MIN cursor de orgs activos). Salvaguarda de escala para Fase C. | Baja — debajo del refactor de migraciones |

---

## 4. Camino recomendado para mañana (orden)

```
1. P5  Refactor migraciones (1783465859144)  ← correctness, desbloquea limpieza DB
   └─ incluye arreglar drizzle_execute all-strings + helper de escrituras con node_id
2. P1  Tests relay/reconnect (cierra Fase 1 al 100%)
3. P2  Binario syntrix-relay (cierra gap NAT del ADR)
4. P6  Pruning turso_cdc (rápido, tras P5)
5. P3  Spike llama-cpp  ← empezar el de-risk BLOQUEANTE de Fase 2
   └─ si el spike pasa → P4 (backend real de inferencia)
```

**Razón del orden**: P5 toca correctness (bug de no-replicación), así que va primero. P1/P2 cierran
Fase 1 (trabajo ya casi completo). P3 es el gran de-risk de Fase 2 y conviene arrancarlo pronto
porque si falla, hay que reconsiderar toda la arquitectura de IA embebida.

---

## 5. Contexto técnico crítico (para no re-descubrir)

- **Compilación**: `export PATH="$PWD/bin:$PATH"; REMOTE_HOST=server-2 cargo check ...`. NO compilar local. server-2 tiene cache caliente + Ollama. server-1 puede tener Nix vacío.
- **libp2p**: ahora en `0.56`. El fork local `~/Documents/github/rust-libp2p` está en `0.57.0-dev` (tags hasta v0.56.0). crates.io máximo = 0.56.0.
- **relay transport**: ✅ ya integrado vía `with_relay_client` en `lib.rs`. El behaviour `relay_client` se inyecta en `with_behaviour(|_key, relay_client| CustomBehaviour {...})`.
- **NO editar `.sql` de migración a mano** — usar `just drizzle-gen` desde `packages/shared-drizzle/src/entities.ts`. (Regla en AGENTS.md + justfile.)
- **Tests binary e2e**: `just test-binary-e2e` (compila remoto, ejecuta local — pipeline automático, sin setup extra).
- **turso fork**: `/home/alcala/Documents/github/turso` — para spikes de CDC (pruning recursivo, change_id monotónico).

---

## 6. Archivos modificados esta sesión (committed en well-sink)

```
crates/syntrix-ai/Cargo.toml                    reqwest a deps
crates/syntrix-ai/src/router.rs                 +DeviceTier/TaskTier/CascadeDecision/strip_think_tags +16 tests
crates/syntrix-network/Cargo.toml               libp2p 0.56, +blake3, +tcp
crates/syntrix-network/src/behaviour.rs         +autonat/relay::client/dcutr
crates/syntrix-network/src/lib.rs               +block_peer/reconnect/scoring/bootstrap/providers, with_relay_client
crates/syntrix-network/tests/block_peer.rs      NUEVO
apps/admin/src-tauri/src/admin.rs               +block_peer en deactivate
apps/admin/src-tauri/src/identity.rs            +TCP listen, +reconnect handler
apps/admin/src-tauri/tests/binary_e2e_test.rs   +3 tests
apps/admin/src-tauri/tests/sync_test.rs         +device_type test
apps/admin/src/__tests__/screens/*.test.tsx     ViewsPage, ModulesPage
apps/client/src-tauri/src/identity.rs           +TCP listen, +reconnect handler
apps/client/src-tauri/src/lib.rs                +drizzle_execute headless
apps/client/src-tauri/tests/ai_views_test.rs    NUEVO 7 tests
apps/client/src-tauri/tests/common/mod.rs       NUEVO helpers
apps/client/src/__tests__/**                    catalog×5, screens×2, VoiceInput, useIAQueue
packages/shared-drizzle/src/entities.ts         FK inválidas removidas
apps/*/src-tauri/migrations/*                    0003 admin, 0004 client (drizzle-gen)
.ai/AGENTS.md                                    refresh completo
apps/docs/src/content/docs/estado-actual.md     +benchmark finding
.kilo/plans/1783125232147-tool-call-parser.md   SUPERSEDED
.kilo/plans/1783367768611-...                    roadmap handover
justfile                                        nota test-binary-e2e
```
