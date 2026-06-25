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
- **`just test`**: Runs client/admin test suites.
- **`just lint`**: Runs TypeScript/Eslint checks.

---

## Directory Structure
- `apps/admin/` — Admin Console application (Tauri + React)
- `apps/client/` — Client/Worker application (Tauri + React)
- `packages/syntrix-ui/` — Shared UI component library (`@syntrix/ui/*`)
- `bin/` — Helper scripts including the `cargo` remote compiler bridge
- `crates/` — Shared Rust libraries (e.g., `iroh-syntrix-docs`)
- `justfile` — Project automation runner

---

## Key Conventions
1. **Shared UI**: Reusable UI components, pages (like `SyncDetailsPage`), and styling should be added to `packages/syntrix-ui/` to ensure parity and consistency between both Admin and Client apps.
2. **Imports**: Apps map `@syntrix/ui/*` directly to the shared package src files in their `tsconfig.json`. Ensure path mappings are updated if you add subfolders.
3. **TypeScript Strictness**: The projects compile with `strict` and `noUnusedLocals` enabled. Ensure function parameters not actively used are prefixed with an underscore (e.g. `_args`) and unused imports are cleaned up before calling the task finished.
