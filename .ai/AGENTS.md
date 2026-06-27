# Syntrix P2P — Context for AI Agents

## Purpose
Monorepo for the Syntrix Peer-to-Peer (P2P) local-first application suite, featuring an admin console and a client application with decentralized synchronization.

## Tech Stack
- **Backend**: Rust + Tauri + Iroh (P2P namespaces, blobs, and sync)
- **Frontend**: React + Vite + Tailwind CSS + Radix/shadcn primitives
- **Monorepo Manager**: `pnpm` workspaces (apps and shared package)
- **Shared UI**: `@syntrix/ui` (under `packages/syntrix-ui/`)

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
- **New entity→namespace mapping** → covered by a multi-node propagation test.
- **New `#[indexed]`/`#[searchable]` field** → covered by query/search tests.

### Running tests
Use `just test-rust` to run all Rust tests via the remote bridge on `server-1`. Do **not** run `cargo test` locally (laptop CPU restriction).

### Test utility crate
`crates/syntrix-testkit/` provides:
- `temp_node_dir()` — isolated temp directories for per-test state.
- `poll_until()` — async polling with exponential backoff for net-dependent assertions.
- `make_invite_ticket()` — generate invite tickets without the admin crate.

### Approval gate
Do not approve a backend feature without a green `just test-rust`.

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
