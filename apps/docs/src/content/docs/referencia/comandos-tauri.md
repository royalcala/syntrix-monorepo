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

---

## Módulo de Live Queries

### `live_subscribe`
- **Descripción**: Suscribe al frontend a una consulta SQL en vivo. Cada vez que los datos subyacentes cambian (vía CDC Sync), el backend re-ejecuta la consulta y emite el resultado actualizado al frontend.
- **Argumentos**:
  - `sql: String` (Consulta SQL).
  - `depends_on: Vec<String>` (Lista de tablas/entidades de las que depende la consulta).
- **Retorno**: `Result<u64, String>` (ID único de la suscripción).

### `live_unsubscribe`
- **Descripción**: Cancela una suscripción activa a una consulta en vivo.
- **Argumentos**:
  - `id: u64` (ID de la suscripción retornado por `live_subscribe`).
- **Retorno**: `Result<(), String>`
