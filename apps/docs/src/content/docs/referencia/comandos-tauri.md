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

---

## Módulo de Consola SQL (Admin)

### `run_sql`
- **Descripción**: Ejecuta una consulta `SELECT`/`WITH` de solo lectura contra la réplica
  relacional del admin, con paginación. Rechaza cualquier sentencia que no sea de lectura
  (`INSERT`/`UPDATE`/`DELETE`/`DROP`/`ALTER`/`CREATE`/`ATTACH`/`PRAGMA`/etc.) y sentencias
  apiladas (`;` intermedio).
- **Argumentos**:
  - `query: String` (una sola sentencia SQL de solo lectura).
  - `limit: Option<usize>` (por defecto 200, tope 1000).
  - `offset: Option<usize>`.
- **Retorno**: `Result<SqlResult, String>` — `{ columns: Vec<String>, rows: Vec<Vec<Value>>, truncated: bool }`.

### `list_saved_views` / `create_saved_view` / `delete_saved_view`
- **Descripción**: CRUD sobre `saved_views` (consultas SQL favoritas). "Audit Trail" se
  siembra automáticamente al iniciar (`sql_console::seed_default_saved_views`).
- **Argumentos** (`create_saved_view`): `name: String`, `sql_query: String` (validado como
  solo-lectura antes de guardar).
- **Retorno**: `Result<SavedView, String>` / `Result<Vec<SavedView>, String>` / `Result<(), String>`.

