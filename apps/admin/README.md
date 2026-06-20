# syntrix-admin

Admin console for Syntrix P2P network.

## Prerequisites (Linux)

```bash
# Debian/Ubuntu
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libsoup-3.0-dev libjavascriptcoregtk-4.1-dev

# Fedora
sudo dnf install gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf

# macOS
xcode-select --install
```

## Development

```bash
pnpm install
cd src-tauri
cargo tauri dev
```

## Structure

```
syntrix-admin/
├── src/                    ← React frontend (admin UI)
│   └── App.tsx             ← Screens: Unlock, Dashboard, Devices, Roles
├── src-tauri/              ← Rust backend
│   ├── src/
│   │   ├── lib.rs          ← Tauri builder + commands
│   │   ├── main.rs         ← Entry point
│   │   ├── identity.rs     ← Device identity (Ed25519 keypair)
│   │   └── admin.rs        ← org_control management (stubs — wires to iroh)
│   ├── Cargo.toml
│   └── tauri.conf.json
└── DESIGN.md               ← Full architecture design
```

## Stack

- Tauri v2 (desktop + iOS + Android)
- React 19 + TypeScript + Vite 7
- iroh-syntrix-docs v0.2.0 (authorization crate)
- iroh + iroh-docs (P2P storage + sync)
