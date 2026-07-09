# Finalización de sesión — Fix transporte TCP + complementar tests

> Rama: `well-sink`. Cierra la sesión 2026-07-08 validando el trabajo hecho (syntrix-migrate,
> upsert_entity, drizzle_execute typed, relay/reconnect, CDC pruning) y desbloqueando la suite de
> integración. Ejecutar con un agente capaz de editar código y correr compilaciones remotas.

## Objetivo

1. Arreglar el transporte TCP faltante en `syntrix-network` — bug real que hace fallar el arranque
   de las apps y bloquea los 23 tests Tier 2 (`sync_test.rs`) + Tier 3 (`binary_e2e_test.rs`).
2. Complementar cobertura de tests para el trabajo de la sesión (syntrix-migrate, upsert_entity,
   drizzle_execute typed).
3. Correr todas las suites y dejar el estado documentado.

## Contexto verificado

- `crates/syntrix-network/src/lib.rs:125` construye el swarm con `.with_quic().with_dns().with_relay_client(...)`
  pero **sin `.with_tcp(...)`**. Ambos apps (`apps/client/src-tauri/src/identity.rs:77`,
  `apps/admin/src-tauri/src/identity.rs:74`) escuchan en `/ip4/0.0.0.0/tcp/0`, así que
  `swarm.listen_on(tcp_addr)?` falla con `Multiaddr is not supported: /ip4/0.0.0.0/tcp/0` y
  `AppState::new_with_data_dir` retorna `Err` → `sync_test.rs`/`binary_e2e_test.rs` panican al spawnear.
- `crates/syntrix-network/Cargo.toml` ya tiene las features `"tcp"`, `"noise"`, `"yamux"`. `noise` y
  `yamux` ya están importados (los usa `with_relay_client`).
- Los tests del crate network (`block_peer`, `reconnect_basic`, `relay_basic`) escuchan solo en QUIC
  (`/ip4/127.0.0.1/udp/0/quic-v1`), por eso pasan hoy; agregar TCP no los afecta.
- `crates/syntrix-migrate/` no tiene tests (`#[cfg(test)]` ausente).
- `upsert_entity_impl` (`apps/client/src-tauri/src/lib.rs:197`) es `pub` y toma `&AppState`.
- `test_drizzle_execute_reads_typed_columns` (`sync_test.rs:749`) solo verifica columnas TEXT.

## Compilación (obligatorio)

`export PATH="$PWD/bin:$PATH"; export RUST_MIN_STACK=16777216; export REMOTE_HOST=server-2`.
No compilar local. Preferir **server-2** (server-1 tiene OOM/GC). Ver `.ai/AGENTS.md`.

---

## Tareas (orden de ejecución)

### G0 — Fix transporte TCP (desbloquea Tier 2/3)

1. En `crates/syntrix-network/src/lib.rs`:
   - Añadir `tcp` al `use libp2p::{autonat, dcutr, noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};`.
   - Insertar el transporte TCP **antes** de `.with_quic()`:
     ```rust
     let mut swarm = SwarmBuilder::with_existing_identity(config.keypair)
         .with_tokio()
         .with_tcp(
             tcp::Config::default(),
             noise::Config::new,
             yamux::Config::default,
         )?
         .with_quic()
         .with_dns()?
         .with_relay_client(noise::Config::new, yamux::Config::default)?
         .with_behaviour(...)?
         .build();
     ```
2. `cargo check -p syntrix-network` y `cargo check -p syntrix-client -p syntrix-admin` en verde.

### G1 — Tests unitarios de `syntrix-migrate`

3. Crear fixtures embebibles:
   - `crates/syntrix-migrate/tests/fixtures/migrations/0000_init.sql` (ej. `CREATE TABLE IF NOT EXISTS t_a (...);`).
   - `crates/syntrix-migrate/tests/fixtures/migrations/0001_add.sql` (ej. `CREATE TABLE IF NOT EXISTS t_b (...);`, con `--> statement-breakpoint` si hay >1 statement).
   - `crates/syntrix-migrate/tests/fixtures/migrations/meta/_journal.json` con 2 entries (idx 0/1, tags matching, `breakpoints: true`).
4. Añadir `tempfile = "3"` a `[dev-dependencies]` de `crates/syntrix-migrate/Cargo.toml`.
5. Añadir `#[cfg(test)] mod tests` en `crates/syntrix-migrate/src/lib.rs`:
   - Helper `test_conn()` (patrón de `cdc.rs`: `PlatformIO` + `Database::open_file_with_flags`).
   - `static FIXTURES: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/migrations");`
   - Construir strings de journal inline (1-entry vs 2-entry) para reusar el mismo `dir`.
   - Casos:
     - `fresh_db_applies_and_records`: journal 2 entries → `SELECT COUNT(*) FROM __migrations` == 2; tablas `t_a`/`t_b` existen.
     - `second_run_is_noop`: correr dos veces el journal de 2 → sigue 2 filas, sin error.
     - `applies_only_pending`: correr journal 1-entry, luego journal 2-entries → `__migrations` pasa de 1 a 2 (solo aplica 0001).
     - `rollback_on_bad_statement`: journal apuntando a un `.sql` con SQL inválido → retorna `Err` y su `tag` **no** queda en `__migrations`.

### G2 — Tests de correctness (en `apps/admin/src-tauri/tests/sync_test.rs`; requieren G0)

6. `test_upsert_entity_stamps_sync_meta`:
   - `spawn_client`, join org, `set_client_org`.
   - `syntrix_client_lib::upsert_entity_impl(&c1, &org_id, "ia_queries", "q1", fields)` con `fields = {text, status:"pending"}`.
   - `drizzle_execute_impl(&c1, "SELECT node_id, change_time FROM ia_queries WHERE org_id=?1 AND doc_id=?2", [org,"q1"])`.
   - Assert: `node_id` == `hex::encode(c1.node_id())` (no vacío) y `change_time` numérico > 0.
7. Extender `test_drizzle_execute_reads_typed_columns`:
   - Escribir una invoice con `amount` numérico vía `commit_event_impl`.
   - `SELECT amount FROM invoices ...` y assert `rows[0][0].is_number()` (no string) — valida el fix `cf9e2c8`.

### G3 — Correr suites y documentar

8. `just test-rust` (server-2). Esperado en verde:
   - `syntrix-migrate` (+4 nuevos), `syntrix-network` (25), `syntrix-ai` (51), `ai_views` (7),
     inline `indexes.rs`/`cdc.rs`, y `sync_test.rs` (23, ahora desbloqueado, +2 nuevos).
9. `just test` (vitest) — esperado 75 en verde (client 49 + admin 26).
10. `just test-binary-e2e` (local, requiere GTK) — smoke de que ambos binarios arrancan con TCP+QUIC.

---

## Riesgos y mitigación

- **Orden del builder**: `.with_tcp()` debe ir antes de `.with_quic()`; ambos comparten `noise`+`yamux`.
  Si el tipo del builder no encadena, revisar firma exacta de `with_tcp` en libp2p 0.56.
- **server-1 OOM**: usar server-2 y `RUST_MIN_STACK=16777216`; si falla, `just kill-remote clean-remote-targets`.
- **`include_dir!` en tests**: requiere fixtures reales commiteados bajo `tests/fixtures/` (path resuelto en compile-time).
- **upsert_entity test acoplado a AppState/P2P**: por eso vive en `sync_test.rs` y depende de G0.
- **binary_e2e**: necesita GTK local; si el entorno no está listo, diferir solo ese paso (documentar).

## Validación (criterio de éxito)

- `just test-rust` en verde **incluyendo** `sync_test.rs` (antes 100% bloqueado).
- 4 tests nuevos de `syntrix-migrate` pasan.
- `test_upsert_entity_stamps_sync_meta` confirma `node_id` no vacío.
- `test_drizzle_execute_reads_typed_columns` confirma columna numérica devuelta como número.
- `just test` (frontend) sigue en 75 verdes.

## Fuera de alcance

- Fase 2 real (P4): `InferenceBackend` + `LlamaCppBackend` + GBNF + `PeerDelegate` + weights P2P.
- Mejora del `reconnect_loop` (hoy hace `Dial(Multiaddr::empty())`; los tests ya lo cubren como no-panic).
- `binary_e2e` en CI (requiere GTK; solo local).

---

## ✅ RESULTADOS DE EJECUCIÓN (2026-07-09)

Sesión completada. Ejecutado en **server-2** (server-1 tiene un problema de hardware, ver abajo).

### Tareas del plan

- **G0 — Fix TCP** ✅ `crates/syntrix-network/src/lib.rs`: agregado `tcp` al `use` y `.with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?` antes de `.with_quic()`. Desbloquea el arranque de apps y toda la suite `sync_test.rs`.
- **G1 — Tests `syntrix-migrate`** ✅ 4 tests nuevos (`fresh_db_applies_and_records`, `second_run_is_noop`, `applies_only_pending`, `rollback_on_bad_statement`) + fixtures en `tests/fixtures/migrations/` + `tempfile` en dev-deps.
- **G2 — Tests correctness `sync_test.rs`** ✅
  - `test_upsert_entity_stamps_sync_meta`: usa `customers` (NO `ia_queries` — `upsert_entity_impl`/`upsert_document_full` solo soporta entidades de negocio vía `entity_meta`; `ia_queries` da "unknown entity"). Confirma `node_id == hex(node_id())` y `change_time > 0`.
  - `test_drizzle_execute_reads_typed_columns`: extendido con invoice `amount` REAL; confirma `rows[0][0].is_number()`.

### 🐞 Bugs reales encontrados y arreglados (desbloqueados por G0)

Estos tests nunca habían corrido (sync_test.rs estaba 100% bloqueado por el bug de TCP), así que exponían bugs latentes de la app:

1. **OOM en `drizzle_execute`** (`apps/client/src-tauri/src/indexes.rs`): el loop leía columnas con índice fijo `for idx in 0..32` y `row.get(idx)`; al exceder las columnas reales, `row.get` devolvía un `Value` basura con longitud de texto gigante → intento de alocar **478 GB** → `SIGABRT`. **Fix**: usar `stmt.num_columns()` (igual que `execute_sql_query`).

2. **Deadlock re-entrante de `db_lock`** (`apps/client/src-tauri/src/indexes.rs`): `apply_cdc_events` toma `db_lock` y llama el closure `perm` → `can_node_write` → `get_members`/`get_roles`, que **re-adquieren `db_lock`** (`std::sync::Mutex` no es reentrante) → deadlock. Ocurría **siempre que un cliente recibía un batch CDC de un peer** (bug crítico de producción, no solo de tests). **Fix**: helpers `query_members`/`query_roles` sin lock; `can_node_write` los usa (su caller ya tiene `db_lock`); los públicos `get_members`/`get_roles` siguen tomando el lock para el resto de callers (commit_event/sync, hilo principal).

### Resultados de suites

| Suite | Resultado |
|-------|-----------|
| `syntrix-migrate` | 4 passed ✅ |
| `syntrix-network` | 28 passed ✅ (incluye relay/reconnect con TCP+QUIC) |
| `syntrix-ai` | 51 passed, 2 ignored ✅ |
| `syntrix-client` (lib + `ai_views_test`) | 27 + 7 passed ✅ |
| `syntrix-admin` (lib + `basic_test`) | 10 + 5 passed ✅ |
| `sync_test.rs` | **24 passed, 0 failed, 2 ignored** ✅ (antes 100% bloqueado) |
| `binary_e2e_test` (binarios reales, procesos separados) | **4 passed** ✅ (compilado en server-2, corrido local con nix shell + GTK) |
| Frontend vitest (`just test`) | client 49 + admin 26 = **75 passed** ✅ |

Nota: admin vitest reporta 2 "errors" de teardown React en `DevicesGridPage.test.tsx` (unhandled passive-effect, pre-existentes, no son fallos de test; no se tocó código frontend).

### Cambios en el bridge (`bin/cargo`)

- Añadido `nixpkgs#glibc.dev` a `DEPS_BASE` (headers C, ej. `errno.h` para `aegis`).
- Detección de cargo/rustc de sistema: si existen (server-1 vía rustup) se usan con `CARGO_BUILD_JOBS=1` + `RUST_MIN_STACK=33554432`; si no (server-2), se usa `nixpkgs#cargo`/`nixpkgs#rustc` con `RUST_MIN_STACK=16777216`.

### ⚠️ Infra: server-1 NO usable para compilar (hardware)

- Síntoma: `rustc`/LLVM crashea con **SIGSEGV/SIGILL** en puntos aleatorios (LLVM inliner, ThinLTO, privacy checker) durante compilación **paralela**. Con `CARGO_BUILD_JOBS=1` NO crashea.
- Descartado software: pasa igual con `nixpkgs rustc 1.95.0` (LLVM 21) y `rustup 1.96.1` (LLVM 22). server-2 es **idéntico** (i5-8500, microcode 0xfa, kernel 6.18.35, non-ECC) y compila bien.
- Conclusión: **RAM defectuosa en server-1** (crashes aleatorios bajo carga paralela + RAM non-ECC = corrupción silenciosa). Recomendación: correr `memtest86+` (reinicio, ~1-2h) y reasentar/reemplazar módulo.
- Fix colateral aplicado: el toolchain rustup de server-1 tenía su intérprete glibc **GC'd** por nix (binarios ENOENT). Reinstalado (`rustup install stable`) + GC root en `/nix/var/nix/gcroots/per-user/root/rustup-glibc` para que el `nix gc` semanal no lo borre. Para infra-core: considerar un GC root permanente del toolchain o pinnear `rustc` a canal estable.

### Fuera de alcance (confirmado, no ejecutado)

- `just test-binary-e2e` **SÍ se ejecutó y pasó** (4/4). La receta hardcodea server-1 para compilar; se corrió manualmente compilando en server-2 (server-1 tiene el problema de HW) y ejecutando el binario de test localmente dentro del `nix shell` (que provee GTK + `WEBKIT_DISABLE_*`). Recomendación: parametrizar `test-binary-e2e` para no hardcodear server-1.
