# Roadmap: Fase A Test Hardening → libp2p NAT → Capability-Tiered Inference

> Ruta de continuación multi-sesión. Secuencia tres workstreams con dependencias, y
> **diseña la cobertura de tests para las tres fases**. No re-especifica los planes de red/IA
> (referencia, no duplica): `1783371801395-libp2p-nat-reconnect-plan.md` y
> `1783452736636-capability-tiered-inference.md`.
>
> **Supersede:** `1783125232147-tool-call-parser.md` queda absorbido por
> `1783452736636-capability-tiered-inference.md` (GBNF elimina el tool-calling frágil). Marcar ese
> archivo como obsoleto en Fase 0.

## Contexto verificado (2026-07-07)

- **Fase A del plan maestro (`1783125232147`) está construida pero NO endurecida**: `cargo test --workspace`
  y `just test` nunca se confirmaron verdes tras todos los edits; server-1 tuvo Nix vacío, server-2 ocupado.
- **Violaciones de convención (AGENTS.md) abiertas por Fase A**:
  - `#[tauri::command]` nuevos sin `*_impl` + test de integración: `ai_chat`, `ai_status`.
  - Entidades nuevas sin test de integración: `view_definitions`, `ia_queries`.
  - `device_type` en `admin_devices`: args actualizados en `sync_test.rs` pero **sin aserción de comportamiento**.
- **Tests existentes de Fase A**: solo unit del crate `syntrix-ai` (62 en bench/lib/provider/router/tools).
  Cero tests frontend para catálogo, HomeScreen, ViewScreen, VoiceInput, useIAQueue, ViewsPage, ModulesPage.
- **Precedente de tests de UI compartida (decisión clave)**: `EntityGrid`/`DetailPanel` viven en
  `packages/syntrix-ui/src/components/` y se testean desde `apps/client/src/__tests__/components/`.
  `packages/syntrix-ui` **no tiene** vitest (sin `test` script, sin dep, sin config). Ambas apps sí
  tienen vitest dentro de `vite.config.ts`. → **Los tests del catálogo van en la vitest de la app consumidora (Opción B)**, no en syntrix-ui.
- **testkit real** (`crates/syntrix-testkit/src/lib.rs`): `temp_node_dir`, `poll_until`, `parse_node_id`,
  `wait_for_sync`, `temp_limbo_db`, `seed_test_document`, `wait_for_document_limbo`, `wait_for_event`,
  `create_entity_table`, `create_event_log_table`. (`make_invite_ticket` que menciona AGENTS.md **ya no existe**.)
- **AGENTS.md está desactualizado**: lista de comandos headless incompleta (falta `ai_chat`/`ai_status`/
  `update_device`/`set_org_role`/`get_org_role`), helpers de testkit erróneos, secciones duplicadas
  ("Tier 3b" ×2, logging ×2), sin mención de crate `syntrix-ai`, catálogo, Shell Universal ni del hallazgo
  del benchmark (modelos locales 0% tool-calling nativo).
- **Compilación**: usar el bridge (`export PATH="$PWD/bin:$PATH"`, `REMOTE_HOST=server-1|server-2`).
  NO compilar local. Tier 3 (binary e2e): `just test-binary-e2e` compila remoto + ejecuta local.
  **No requiere configuración adicional** — `just` se encarga de todo el pipeline.

## Decisiones resueltas

| # | Decisión | Elección |
|---|----------|----------|
| D1 | Orden de workstreams | Fase 0 (endurecer Fase A) → Fase 1 (libp2p NAT) → Fase 2 (capability-tiered inference) |
| D2 | Ubicación tests de catálogo/UI compartida | **Opción B**: vitest de la app consumidora (`apps/client`, `apps/admin`), siguiendo el precedente de `EntityGrid`/`DetailPanel`. NO añadir vitest a `syntrix-ui`. |
| D3 | tool-call-parser | Superseded por capability-tiered (GBNF). Marcar obsoleto. |
| D4 | AGENTS.md desactualizado | Refrescar en Fase 0 (comandos headless, testkit, crate syntrix-ai, entidades, catálogo, hallazgo benchmark, quitar duplicados). |
| D5 | e2e de calidad de generación IA | **Gated** a Fase 2 (requiere GBNF). En Fase 0 solo se testea el *plumbing* (comandos, CDC-sync de vistas, render con data mock). |
| D6 | Gates de aprobación | Sin `just test-rust` verde no se aprueba backend. Features de invite/sync/delegación además requieren `just test-binary-e2e` verde. |

---

## FASE 0 — Endurecer Fase A (bloquea Fases 1–2)

Objetivo: build verde + cerrar violaciones de convención + cobertura de lo ya construido.

### 0.1 Confirmar build/estado verde
1. `export PATH="$PWD/bin:$PATH"; REMOTE_HOST=server-1 cargo test --workspace` → verde. Arreglar lo que rompa.
2. `just test` (vitest admin+client) → verde. `just lint` → verde.
3. `just test-binary-e2e` (local, GTK) → sin regresión de invite/sync/CDC.

### 0.2 Tests Rust — Tier 1/2 (integración con `temp_limbo_db` + `seed_test_document`)
4. **`view_definitions` CRUD + migración** — nuevo test (p.ej. `apps/client/src-tauri/tests/ai_views_test.rs`
   o inline en `indexes.rs`): aplica migración `0003`, inserta vista, `SELECT` por `org_id`, verifica columnas
   (`sql, entity, components_json, root, meta_json, created_by, tags`) y LWW (`change_time`/`node_id`).
5. **`ia_queries` CRUD + transición de estado** — insert `pending` → update `completed`+`view_id` → `seen`.
   Verifica índice por `(org_id, status)`.
6. **`AiContext for AppState`** — `save_view`/`list_views` contra Limbo real: `save_view` persiste y
   `list_views` filtra por `org_id`; `query_entity` valida SELECT-only (rechaza DELETE/UPDATE/INSERT);
   `search_entity` mapea `search::SearchResult`→`tools::SearchResult`; `check_read_access` se aplica.
7. **`ai_chat_impl` end-to-end con `MockProvider`** (ya existe unit; elevar a integración con `AppState` real):
   tool-call → `execute_tool` → `save_view` persiste en `view_definitions`; loop corta a ≤10 rounds;
   status `Generated`.
8. **`device_type`** — en `sync_test.rs`/`basic_test.rs`: tras `update_device(..., device_type=Some("client-ia"))`,
   `list_org_devices` devuelve `device_type == "client-ia"`; default es `"client"` al crear device.

### 0.3 Tests Rust — Tier 3 (binary e2e, `--headless`)
9. Extender `apps/admin/src-tauri/tests/binary_e2e_test.rs`:
   - `ai_status` (client) → responde modelos + health (sin requerir Ollama: health puede degradar).
   - `ai_chat` (client) con Ollama ausente → error/deg. controlado (no panic); si se mockea, ver 0.2.7.
   - **CDC-sync de `view_definitions`**: peer A `save_view` → peer B ve la vista vía CDC (patrón del test de gossip existente).
   - `update_device` con `device_type` → propaga y `list_devices` refleja el tipo.

### 0.4 Tests Frontend — vitest (Opción B, en la app consumidora)
10. **Catálogo** (`apps/client/src/__tests__/components/catalog/`): render con data mock de
    `DataTable`, `MetricCard` (currency/number/percentage), `EntityDetail` (fields+relaciones),
    `Form` (validaciones required), `Column`/`Row`/`Card`, y `ViewRenderer` (adjacency list recursiva,
    root inválido → mensaje de error).
11. **HomeScreen**: input + envío llama `ai_chat` (mock invoke); vistas recientes render; fallo de `ai_chat`
    → `queueIAQuery` + toast. **ViewScreen**: carga `ViewDefinition`, ejecuta SQL (mock `drizzle_execute`),
    pasa data a `ViewRenderer`. **VoiceInput**: sin SpeechRecognition → no renderiza; con mock → `onTranscript`.
    **useIAQueue**: fila `completed` → toast + invalida caché + marca `seen`.
12. **Admin**: `ViewsPage` (lista/filtra/acciones con mock), `ModulesPage` (placeholder render),
    `DevicesGridPage` (columna `device_type` + botón "Designar IA" invoca `update_device` con `device_type:"client-ia"`).

### 0.5 Docs
13. **Refrescar `.ai/AGENTS.md`**: comandos headless reales (incl. `ai_chat`/`ai_status`/`update_device`/
    `set_org_role`/`get_org_role`), helpers de testkit reales (quitar `make_invite_ticket`, añadir
    `temp_limbo_db`/`seed_test_document`/`wait_for_document_limbo`/`wait_for_event`), sección `syntrix-ai`
    (crate, `ai_chat`/`ai_status`, entidades `view_definitions`/`ia_queries`, `device_type`, catálogo,
    Shell Universal), hallazgo benchmark (0% tool-calling local → GBNF en Fase 2), eliminar secciones duplicadas.
14. **`apps/docs/src/content/docs/`**: actualizar `estado-actual` con Fase A completa + hallazgo de modelos.
15. Marcar `1783125232147-tool-call-parser.md` como **SUPERSEDED por 1783452736636** (nota al inicio).

**Gate Fase 0**: `just test-rust` + `just test` + `just test-binary-e2e` verdes.

---

## FASE 1 — libp2p NAT Traversal + Auto-Reconexión

Ejecutar el plan `1783371801395-libp2p-nat-reconnect-plan.md` (autonat/relay/dcutr, TCP fallback,
backoff reconnect, Kademlia discovery por org, bootstrap nodes, re-announce, `block_peer`, peer scoring).

### Diseño de tests Fase 1
- **Tier 1 (unit)**: cálculo de backoff (2→64s), peer scoring (`disconnect_count`→delay inicial),
  key Kademlia = `blake3("syntrix-p2p-"+org_id)`.
- **Tier 2 (integración, `crates/syntrix-network/tests/`)**:
  - `reconnect_basic.rs`: dos `P2PNode`, desconexión → `ensure_connected` restablece; scoring acelera peer estable.
  - `relay_basic.rs`: conexión vía relay entre nodos sin ruta directa (usar `poll_until`).
  - `block_peer.rs`: `block_peer()` cierra conexión y previene re-dial.
- **Tier 3 (binary e2e)**: sin regresión de invite/catchup/CDC; `block_peer` desde admin desconecta al client.
- **No-regresión (gate)**: `just test-rust` + `just test-binary-e2e` verdes; features nuevos de libp2p
  no rompen el build (`just build`).

**Gate Fase 1**: tests arriba verdes + prueba manual (dos peers en redes separadas/VPN conectan vía relay;
reconexión <2 min; re-descubrimiento tras cambio de IP).

---

## FASE 2 — Cascada de Inferencia por Capacidad

Ejecutar el plan `1783452736636-capability-tiered-inference.md`. **Tarea 1 (spike de-risk llama-cpp-2 en
Nix + NDK Android + iOS) es BLOQUEANTE**: no avanzar sin ella.

### Diseño de tests Fase 2
- **Spike (Tarea 1)**: compila y genera con GBNF en Linux (Nix `rustPlatform.bindgenHook`), Android
  (`pkgsCross.aarch64-android`/`cargo-ndk`) e iOS (`pkgsCross.iphone64`). Documentar el build que funcione.
- **Tier 1 (unit)**: `DeviceTier` desde specs+tok/s; `TaskTier` (`TaskType`→tier mínimo); router de cascada
  `(DeviceTier × TaskTier) → {Local|Delegate|Cloud|Unavailable}`; saneo `<think>…</think>`.
- **Tier 2 (integración)**:
  - **GBNF `ViewDefinition`**: la gramática produce JSON válido al esquema en 100% de casos con granite/qwen/
    deepseek reales (parsea a `ViewDefinition`, columnas existen). Reemplaza al benchmark de tool-calling (0%).
  - `InferenceBackend`: `LlamaCppBackend` genera single-shot con gramática; `OllamaBackend` (adaptado) sigue verde;
    `MockProvider` para lógica de cascada sin modelo.
  - **Cascada degrada** en 3 escenarios: device fuerte→Local; débil con peer→Delegate (request-response);
    débil sin peer→Cloud opt-in / notifica `Unavailable`.
  - **Advertising de capacidad vía CDC**: probe escribe tier/modelos/salud → visible org-wide → device débil lo lee.
- **Tier 3 (binary e2e)**: `PeerDelegateBackend` — device débil delega a peer capaz vía request-response;
  offline→cola `ia_queries`→peer procesa→responde vía CDC (`check_read_access` aplicado).
  Distribución P2P de weights: transferencia chunked GGUF peer→peer con verificación de hash y reanudación.
- **Tier 4 (Playwright, gated)**: flujo voz→vista end-to-end con GBNF; CDC-sync de la vista generada entre peers.
  (Este es el e2e de calidad de generación diferido de Fase 0.)

**Gate Fase 2**: spike documentado + GBNF 100% válido + cascada 3-escenarios + weight transfer verificado +
regresión (6 entidades fallback, `just test-rust` + `just test`).

---

## Matriz de cobertura (resumen)

| Superficie | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---|---|---|---|---|
| `syntrix-ai` core (router/tools/provider) | ✅ existe | 0.2.6-7 | — | — |
| `ai_chat`/`ai_status` commands | — | 0.2.7 | 0.3.9 | F2 |
| `view_definitions`/`ia_queries` | — | 0.2.4-5 | 0.3.9 (CDC) | — |
| `device_type` | — | 0.2.8 | 0.3.9 | 0.4.12 |
| Catálogo + ViewRenderer | — | — | — | 0.4.10 (vitest) |
| Home/View/Voice/useIAQueue | — | — | — | 0.4.11 (vitest) |
| Admin Views/Modules/Devices | — | — | — | 0.4.12 (vitest) |
| libp2p NAT/reconnect/block | F1 unit | F1 integ | F1 e2e | manual |
| InferenceBackend/GBNF/cascada | F2 unit | F2 integ | F2 e2e | F2 e2e |

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Build de Fase A rojo al confirmar | Fase 0.1 primero; arreglar antes de añadir tests. |
| Ollama ausente en CI para tests de IA | Tier 1/2 usan `MockProvider`; GBNF/modelos reales solo en Fase 2 con server-2. |
| Spike llama-cpp móvil falla | BLOQUEANTE en Fase 2; no comprometer arquitectura sin él (ya en plan `1783452736636`). |
| NAT es dependencia blanda de delegación | LAN funciona sin NAT; Fase 1 antes de Fase 2 endurece delegación cross-NAT. |
| `packages/syntrix-ui` sin vitest tienta a añadir infra | Decisión D2: testear desde la app consumidora (precedente EntityGrid/DetailPanel). |
| server-1 Nix vacío / server-2 ocupado | Bridge con `REMOTE_HOST` seleccionable; poblar cache una vez. |

## Validación global
1. Fase 0: `just test-rust` + `just test` + `just test-binary-e2e` verdes; AGENTS.md/docs refrescados.
2. Fase 1: tests de red verdes + prueba manual NAT/relay/reconexión.
3. Fase 2: spike documentado + GBNF 100% + cascada 3-escenarios + weight transfer + regresión.
4. Cada fase respeta el gate de aprobación (D6) antes de pasar a la siguiente.

## Fuera de alcance
- Implementación de código (cambiar a un agente de implementación por fase).
- Reescribir los planes `1783371801395` y `1783452736636` (se referencian).
- Fase B/C del plan maestro (módulos, hardware, inter-org).
