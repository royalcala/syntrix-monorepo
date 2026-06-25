---
title: "Comandos de Tauri"
description: "Referencia de la API de comunicación IPC (comandos de Rust) en Syntrix."
---

# Comandos de Tauri

Esta sección contiene el catálogo oficial de comandos del backend en Rust que pueden ser invocados desde la aplicación de frontend de React.

## Módulo de Sincronización

### `sync_state`
- **Descripción**: Solicita el estado de sincronización actual del nodo P2P con respecto a un namespace específico.
- **Argumentos**:
  - `namespace_id: String` (El ID del namespace criptográfico).
- **Retorno**: `Result<SyncStatus, String>`

---

## Módulo de Identidad

### `get_identity`
- **Descripción**: Retorna la información de identidad pública y configuración del nodo local.
- **Argumentos**: Ninguno.
- **Retorno**: `Result<NodeIdentity, String>`
