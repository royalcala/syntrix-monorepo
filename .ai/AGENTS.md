# Syntrix P2P — Context for AI Agents

## Purpose
Monorepo for the Syntrix Peer-to-Peer (P2P) local-first application suite, featuring an admin console and a client application with decentralized synchronization.

## Tech Stack
- **Backend**: Rust + Tauri + libp2p (P2P networking via syntrix-network crate)
- **Database**: Limbo (`turso_core`) — SQL embebido + FTS nativo + CDC (Change Data Capture)
- **Migrations**: Drizzle ORM (TypeScript schemas → `.sql` generado por `drizzle-kit`)
- **Frontend**: React + Vite + Tailwind CSS + Radix/shadcn primitives
- **Monorepo Manager**: `pnpm` workspaces (apps and shared package)
- **Shared UI**: `@syntrix/ui` (under `packages/syntrix-ui/`)
- **Shared Drizzle entities**: `@syntrix/shared-drizzle` (under `packages/shared-drizzle/`)

---

## Remote Compilation Guidelines (CRITICAL)

> [!IMPORTANT]
> The developer's laptop has limited CPU/memory resources. **DO NOT run heavy Cargo compilations or tests locally** (e.g. standard `cargo build`, `cargo check` or `cargo run` without the bridge). Doing so will lock up the machine.

### The Cargo Bridge
The monorepo contains a custom compiler bridge script at `bin/cargo`. It intercepts `build`, `check`, `clippy`, and `run` commands and redirects them to remote ThinkCentre build servers (`server-1` or `server-2`) via SSH and rsync.

#### How to use the bridge:
1. **Prepend `bin/` to the PATH**: Before running any Cargo command, make sure to add the repository's `bin/` directory to the shell's `PATH`:
   ```bash
   export PATH="$PWD/bin:$PATH"
   ```
2. **Select the remote host**: By default, it delegates to `server-1`. You can override this using the `REMOTE_HOST` variable:
   ```bash
   export REMOTE_HOST="server-1" # or server-2
   ```
3. **Run your command**: Invoking `cargo check`, `cargo build`, etc., will automatically sync files, build on the server, and pull back the `target/` directory:
   ```bash
   cargo check
   ```

### Justfile Tasks
Use the predefined tasks in the `justfile` for running/testing:
- **`just remote-compile-admin`**: Compiles the admin Tauri app on `server-1` and runs the UI shell locally.
- **`just remote-compile-client`**: Compiles the client Tauri app on `server-2` and runs the UI shell locally.
- **`just test`**: Runs client/admin frontend test suites.
- **`just test-rust`**: Runs all Rust tests via the bridge (`cargo test --workspace` on `server-1`).
- **`just lint`**: Runs TypeScript/Eslint checks.
- **`just drizzle-gen`**: Regenerates SQL migration files from Drizzle TypeScript schemas.
- **`just kill-local`**: Kills all local Tauri apps, Vite dev servers, and bridge processes.
- **`just kill-remote`**: Kills compilation processes on `server-1` and `server-2` via SSH.
- **`just clean-remote-targets`**: Removes leftover `target-*` session directories on remote servers.
- **`just kill-all`**: Runs `kill-local` + `kill-remote` + `clean-remote-targets`.

### The Cargo Bridge — Lock & Session Isolation
The bridge at `bin/cargo` includes two concurrency mechanisms to handle multiple AI sessions compiling simultaneously:

1. **Global `flock` lock** (`/tmp/syntrix-bridge-sync.lock`): Serializes the `rsync --delete` of source code to the remote server. If two sessions sync at the same time, the second waits for the first to finish, preventing file deletion races.

2. **Session-isolated target directories**: Each invocation generates a unique `SESSION_ID` (hash of PID + timestamp + cwd) and compiles remotely into `target-$SESSION_ID/` via `CARGO_TARGET_DIR`. This means two sessions never compete for Cargo's artifact lock and can compile in **full parallel**. After a successful build, the binary is downloaded to the local `target/` and the remote session target is deleted.

Bridge log messages include the session identifier for debugging: `[Bridge:a1b2c3d4]`.

### Server Disk Space
`server-1` may run out of disk space (currently ~234GB, often at 100%), triggering nix auto-GC on every build. If compilations are slow or fail with derivation errors, run `just kill-remote clean-remote-targets` and then manually free space on the server:
```bash
ssh server-1 "nix store gc --extra-experimental-features 'nix-command flakes'"
```

---

## Directory Structure
- `apps/admin/` — Admin Console application (Tauri + React)
- `apps/client/` — Client/Worker application (Tauri + React)
- `packages/syntrix-ui/` — Shared UI component library (`@syntrix/ui/*`)
- `packages/shared-drizzle/` — Shared Drizzle schemas + `drizzle-zod` helpers
- `bin/` — Helper scripts including the `cargo` remote compiler bridge
- `crates/` — Shared Rust libraries (e.g., `syntrix-core`)
- `justfile` — Project automation runner

---

## Key Conventions
1. **Shared UI**: Reusable UI components, pages (like `SyncDetailsPage`), and styling should be added to `packages/syntrix-ui/` to ensure parity and consistency between both Admin and Client apps.
2. **Imports**: Apps map `@syntrix/ui/*` directly to the shared package src files in their `tsconfig.json`. Ensure path mappings are updated if you add subfolders.
3. **TypeScript Strictness**: The projects compile with `strict` and `noUnusedLocals` enabled. Ensure function parameters not actively used are prefixed with an underscore (e.g. `_args`) and unused imports are cleaned up before calling the task finished.

---

## Headless Testability (Rust backends)

Every `#[tauri::command]` **must** be a thin wrapper around a public `*_impl(state: &AppState | &mut AppState, ...) -> Result<T, anyhow::Error>` function with no Tauri types. This enables headless testing via `cargo test` without a GUI.

### Convention
- **New `#[tauri::command]`** → expose a pure `*_impl` function + integration test.
- **New entity/table** → includes migration SQL (via Drizzle), SqlEngine methods, and an integration test.

### Running tests
Use `just test-rust` to run all Rust tests via the remote bridge on `server-1`. Do **not** run `cargo test` locally (laptop CPU restriction).

### Test Tiers

| Tier | Type | What it tests | How to run | Catches |
|------|------|---------------|------------|---------|
| **1** | Unit (inline `#[cfg(test)]`) | Pure functions, no I/O | `just test-unit` | Logic bugs |
| **2** | Integration (in-process) | Admin + client sharing same tokio runtime, P2P via localhost | `just test-integration` | P2P protocol, permissions, sync |
| **3** | Binary E2E (separate processes) | Real binaries launched as subprocesses, communicate via stdin/stdout `--headless` mode | `just test-binary-e2e` | Tauri command wrappers, dial, event emission, process lifecycle |
| **4** | Playwright E2E (cross-app UI) | 3 Tauri apps running simultaneously with real WebViews | `scripts/start-e2e-cross-app.sh` + `just test-e2e-cross-app` | Frontend rendering, invite inbox UI, full user flow |

### Tier 3 — Binary E2E tests (`just test-binary-e2e`)
- Requires GTK on the local machine (must run locally, not on server-1).
- Compiles both `syntrix-admin` and `syntrix-client` remotely, downloads binaries, then runs `cargo test --test binary_e2e_test -- --ignored` locally.
- Uses `--headless` mode: both apps read JSON commands from stdin and write JSON responses to stdout.
- Tests: create org → create role → get client address → send invite (with real P2P dial) → check inbox → accept invite → write records → verify bidirectional gossip sync.

### Tier 3b — Timeline CDC (`get_updates_since` + `useTimelineCursor`)
Not a separate test binary, but a backend + frontend feature tested via inline tests:
- **Backend**: `get_updates_since(sinceChangeId)` Tauri command reads `turbo_cdc` from a cursor
  and returns new events + `max_change_id`. Verified by `read_events_since_cursor_timeline`
  and `read_events_since_respects_limit` in `cdc.rs`.
- **Frontend**: `useTimelineCursor` hook in `apps/client/src/hooks/useTimelineCursor.ts`
  polls every 2s, stores cursor in localStorage, and invalidates `react-query` entity caches
  when new CDC events arrive.

### Tier 4 — Playwright E2E tests
- See `apps/admin/src/__tests__/e2e-cross-app/` for the cross-app test.
- Requires 3 Tauri apps running with `--remote-debugging-port` and WebDriver.
- Start apps: `bash scripts/start-e2e-cross-app.sh`
- Run test: `just test-e2e-cross-app`

### Headless mode (`--headless`)
Both `syntrix-admin` and `syntrix-client` support a `--headless` CLI flag that:
- Skips the Tauri WebView
- Initializes `AppState` with P2P networking
- Reads JSON commands from stdin: `{"cmd": "...", "arg": "..."}`
- Writes JSON responses to stdout: `{"ok": true, "data": ...}`
- Supported admin commands: `create_org`, `create_role`, `list_roles`, `send_invite`, `get_endpoint_addr`, `list_orgs`, `list_devices`, `ping`
- Supported client commands: `get_invites`, `get_endpoint_addr`, `join_org`, `list_orgs`, `set_active_org`, `commit_event`, `query_entity`, `ping`
- The `send_invite` command includes the full dial flow (add_device → dial all addresses → wait 1s for QUIC handshake → send invite via request-response).

### Test utility crate
`crates/syntrix-testkit/` provides:
- `temp_node_dir()` — isolated temp directories for per-test state.
- `poll_until()` — async polling with exponential backoff for net-dependent assertions.
- `make_invite_ticket()` — generate invite tickets without the admin crate.

### Approval gate
Do not approve a backend feature without a green `just test-rust`.
- For invite/sync features: also require green `just test-binary-e2e`.

---

## Relational Data Model & CDC-Native Sync

Entity tables use **typed SQL columns** (not a generic `payload` JSON blob) — see
`.kilo/plans/1782949593655-relational-cdc-migration.md` for the full migration history and
rationale.

### Tier 3b — Timeline CDC (`get_updates_since` + `useTimelineCursor`)
Not a separate test binary, but a backend + frontend feature verified by inline tests:
- **Backend**: `get_updates_since(sinceChangeId)` Tauri command reads `turbo_cdc` from a cursor
  and returns new events + `max_change_id`. Verified by `read_events_since_cursor_timeline`
  and `read_events_since_respects_limit` in `cdc.rs`.
- **Frontend**: `useTimelineCursor` hook in `apps/client/src/hooks/useTimelineCursor.ts`
  polls every 2s, stores cursor in localStorage, and invalidates `react-query` entity caches
  when new CDC events arrive. This replaces `event_log`-based polling and provides near-real-time
  updates without `LiveManager` or WebSockets.
- Tests: 17/17 CDC tests pass (including 2 timeline cursor tests).

### Tier 3c — Drizzle Proxy (`drizzle_execute`)
- **What**: Frontend can use Drizzle ORM's full type-safe query API (select, joins, where,
  aggregations) via `drizzle-orm/sqlite-proxy`. Drizzle generates SQL on the frontend,
  sends it to the Rust backend via `invoke("drizzle_execute", { sql, params })`,
  and the backend executes it against Limbo via `conn.prepare() + stmt.step()`.
- **Backend**: `drizzle_execute_impl` in `apps/client/src-tauri/src/lib.rs` — binds params,
  iterates rows, returns `{ rows: [[value, ...], ...] }`.
- **Frontend**: `apps/client/src/db.ts` exports a `db` instance backed by the proxy.
  Example: `const rows = await db.query.customers.findMany({ where: eq(customers.name, "Acme") })`.
- **Schema**: imports from `packages/shared-drizzle/src/entities` for full Drizzle types.
- Test: `test_drizzle_execute_reads_typed_columns` in sync_test.rs verifies SELECT with params
  returns correct rows.
- **Writes still go through `commit_event`** (permissions + HLC + CDC). Reads via Drizzle
  Proxy bypass `can_open` (non-security concern — CDC layer enforces real permissions). This replaces `event_log`-based polling and provides near-real-time
  updates without `LiveManager` or WebSockets.
- Tests: 17/17 CDC tests pass (including 2 timeline cursor tests).

### Adding/changing an entity's fields
1. Edit the entity's columns in `packages/shared-drizzle/src/entities.ts`
   (both apps import from this shared location, so one edit covers both).
2. Edit the matching column metadata (`type`, `nullable`, `searchable`) in
   `shared/drizzle/entity-schema-meta.mjs` — this is the single source of truth for the column
   registry consumed by Rust.
3. Run `just drizzle-gen` to regenerate SQL migrations (Drizzle Kit) **and**
   `crates/syntrix-network/schema.json` (via `pnpm export-schema`). Do not hand-edit
   `schema.json` — it's generated.
4. Rust reads the registry via `syntrix_core::schema` (re-exported from
   `syntrix-network::schema`, which lives there to avoid a circular crate dependency). Never
   hardcode per-entity column lists in Rust — use `entity_meta`/`business_columns`/
   `child_business_columns`/`searchable_columns`.
5. Dev DBs are not migrated/backfilled across schema changes — reset with
   `just clean-data-admin` / `just clean-data-client` / `just clean-data-all`.

### CDC-native sync (not JSON-over-gossip)
- Entity data no longer propagates via a direct gossip publish of a JSON business event.
  Writes go through `SqlEngine::upsert_document_full` (typed columns), and
  `apps/client/src-tauri/src/cdc_sync.rs::run_cdc_publish_loop` reads `turso_cdc` periodically
  and gossips `CdcEvent` batches (`syntrix_network::cdc::{read_cdc_events, apply_cdc_events}`).
- `turso_cdc.change_type`: `1`=insert, `0`=update, `-1`=delete. `change_type == 2` is a
  per-transaction **commit marker** (`table_name IS NULL`), not a delete — already handled in
  `read_cdc_events`, but don't reintroduce a `change_type == 2` → delete assumption.
- Permission checks on received CDC events use SQL `members`/`roles` tables
  (`SqlEngine::can_node_write`), **not** the in-memory `NamespaceRegistry` on the client side —
  a client's registry only knows its own device, never peers'. The admin's registry *is*
  authoritative (it issues every `device.updated`/`role.updated`).
- FTS uses turso's `fts_match`/`fts_score` scalar functions directly on typed columns (Tantivy
  under the hood) — there is no SQLite-style `CREATE VIRTUAL TABLE ... USING fts5` support in
  the vendored `turso_core`.
- Catch-up (`apps/client/src-tauri/src/catchup.rs`, admin's `CatchupRequestReceived` handler)
  sends a **relational snapshot** (`syntrix_network::cdc::snapshot_org_rows` → a `cdc_batch`
  of current rows, applied via `apply_cdc_events`) plus the device/role roster — not an
  `event_log` replay. There is no `hlc_tracker` table and no `upsert_document_with_hlc`
  anymore; `change_time` on each row is the only LWW source of truth, for both live CDC and
  catch-up. Don't reintroduce an HLC-based document write path — if you find yourself wanting
  one, it almost certainly means a gossip/catchup code path isn't reusing
  `syntrix_network::cdc::apply_cdc_events` like it should.

### Admin SQL console
- `apps/admin/src-tauri/src/sql_console.rs` exposes `run_sql` (read-only, validated
  SELECT/WITH only) and `saved_views` CRUD. `AuditTrail.tsx` is now a thin wrapper around
  `SqlConsole.tsx` seeded with a query over `event_log`, not a bespoke filter UI.

---

## AI-First Logging (syntrix-logging)

The crate `crates/syntrix-logging/` provides structured NDJSON logging with a query API designed for AI consumption.

### Conventions
- **Log format**: Every event is a `LogRecord` with `{ts, level, target, span_path, corr_id, message, fields}` — written as NDJSON (`logs/syntrix-<app>.ndjson`) and retained in a 5000-entry ring buffer.
- **Canonical operations must use `syntrix_span!`**:
  ```rust
  let (_guard, _entered) = syntrix_span!(org, op, step);
  ```
  Available operations: `create_org`, `send_invite`, `join_org`, `commit_event`, `sync_push`, `sync_pull`, `heartbeat`, `subscribe_ingest`. Each macro creates a tracing span with `org`, `op`, `step`, and auto-generated `corr_id` (UUID). Event fields inherit the span context.
- **Zero secrets in logs**: Sensitive fields (`secret_*`, `keypair`, `doc_ticket`, `ticket`) are auto-redacted. Never log keypairs or invite tickets directly.
- **AI diagnosis**: Use `query_logs` (filtered/paginated) and `summarize_logs` (digest) instead of reading raw log files.
- **Verbosity**: Control via `RUST_LOG` (default `syntrix=info,iroh=warn`) and `IROH_DEBUG=1` (iroh debug to separate file); no code changes needed.
- **Ring buffer**: Default 5000 entries, tunable via `SYNTRIX_LOG_RING` env var.
- **AI diagnosis**: Use `query_logs` (filtered/paginated) and `summarize_logs` (digest) instead of reading raw log files.
- **Verbosity**: Control via `RUST_LOG` (default `syntrix=info,iroh=warn`) and `IROH_DEBUG=1` (iroh debug to separate file); no code changes needed.
- **Ring buffer**: Default 5000 entries, tunable via `SYNTRIX_LOG_RING` env var.
- **New `#[tauri::command]`**: Wrap `*_impl` functions in both apps; new logging APIs follow the same convention.
- **Tail**: The Tauri event `log_event` emits new records in ~250ms batches. UI subscribes via `listen("log_event", ...)`.
- **No Tokio assumptions in `init_logging`**: `init_logging()` may be called before the Tauri runtime starts. Never `tokio::spawn` unconditionally — use `tokio::runtime::Handle::try_current()` to check for an active runtime first.
