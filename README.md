# Syntrix Monorepo

ERP descentralizado P2P. Sin servidores. Sin VPN. Offline-first.

## Estructura

```
apps/
├── client/          ← App empleados (React + Tauri)
└── admin/           ← App admin (React + Tauri)
packages/
└── syntrix-ui/      ← Componentes compartidos (@syntrix/ui)
crates/
└── syntrix-core/      ← P2P networking layer (Rust)
syntrix-docs/        ← Documentación de diseño
```

## Stack

| Capa | Qué usamos |
|------|-----------|
| Shell | Tauri v2 |
| UI | React 19 + Vite 7 + shadcn/ui |
| Grid | TanStack Table |
| State | TanStack DB |
| Forms | TanStack Form + Zod |
| Sync P2P | libp2p gossipsub |
| Auth | syntrix-core |
| Red | libp2p (QUIC, relay) |

## Desarrollo

```bash
pnpm install           # workspace install
cd apps/client && cargo tauri dev
cd apps/admin && cargo tauri dev
```

O usar `just client` / `just admin`.

## Documentación

Ver [`syntrix-docs/`](./syntrix-docs/) — identidad, arquitectura, plan maestro, roadmap UI.
