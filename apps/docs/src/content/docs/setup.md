---
title: "Instalación y Setup"
description: "Cómo instalar y configurar el entorno de desarrollo de Syntrix."
---

# Instalación y Setup

## Requisitos Previos

- **Nix / NixOS** (con comandos experimentales activados para Flakes)
- **Node.js 22+** y **pnpm 11+**
- **Rust y Cargo** (vía el bridge remoto o toolchain local)
- **SSH** a `server-1` / `server-2` (para compilación remota)

## Compilación Remota (Bridge)

> [!IMPORTANT]
> La laptop del desarrollador tiene recursos limitados. **NO compilar Cargo localmente** sin el bridge.

El monorepo incluye un bridge en `bin/cargo` que redirige `build`, `check`, `clippy`, `run`, `test` y `doc` a servidores remotos:

```bash
# Preparar el entorno
export PATH="$PWD/bin:$PATH"
export REMOTE_HOST="server-1"  # o server-2

# Compilar (se ejecuta en el servidor remoto)
cargo check
cargo test
```

## Tareas del Justfile

| Tarea | Descripción |
|-------|-------------|
| `just remote-compile-admin` | Compila admin en server-1, ejecuta UI local |
| `just remote-compile-client` | Compila client en server-2, ejecuta UI local |
| `just test` | Ejecuta tests de client y admin |
| `just lint` | Ejecuta TypeScript/ESLint |
| `just docs` | Levanta el servidor de documentación (Astro) |
| `just clean-data-all` | Borra datos persistidos de ambas apps |

## Estructura del Monorepo

```
syntrix-monorepo/
├── apps/
│   ├── admin/          ← Admin Console (Tauri + React)
│   ├── client/         ← Client/Worker (Tauri + React)
│   └── docs/           ← Documentación (Astro Starlight)
├── packages/
│   └── syntrix-ui/     ← Componentes UI compartidos (@syntrix/ui)
├── crates/
│   └── syntrix-core/   ← P2P auth, sync, heartbeats
├── bin/                ← Scripts (cargo bridge)
└── justfile            ← Automatización
```
