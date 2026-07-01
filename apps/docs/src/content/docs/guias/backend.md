---
title: "Desarrollo Backend (Rust & Tauri)"
description: "Guía de desarrollo para el core de Rust y comandos en Tauri de Syntrix."
---

# Desarrollo Backend

El núcleo de Syntrix está construido en **Rust**, el cual provee la seguridad de memoria, concurrencia y velocidad requeridas para el motor P2P descentralizado.

## Tecnologías Utilizadas

- **Rust (edición 2021)**: Lenguaje del core.
- **Tauri v2**: Framework para conectar el backend de Rust con la UI web.
- **libp2p**: Protocolo P2P de red y comunicación entre pares.
- **turso_core**: Motor SQL embebido (fork de Limbo, basado en SQLite).
- **Drizzle Kit**: Migraciones de esquemas SQL.
- **Cargo**: Gestor de paquetes de Rust.

---

## Agregar un Nuevo Comando en Tauri

Para exponer una función de Rust al frontend de JavaScript, sigue estos pasos:

### 1. Definir la función en Rust
En el archivo `src/lib.rs` (o el módulo correspondiente de tu aplicación Tauri):

```rust
#[tauri::command]
pub fn mi_nuevo_comando(valor: String) -> Result<String, String> {
    if valor.is_empty() {
        return Err("El valor no puede estar vacío".into());
    }
    Ok(format!("Procesado: {}", valor))
}
```

### 2. Registrar el comando
En el builder de la app Tauri:

```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        mi_nuevo_comando
    ])
```

### 3. Invocar desde JavaScript
```typescript
import { invoke } from '@tauri-apps/api/core';

const resultado = await invoke<string>('mi_nuevo_comando', { valor: 'Hola' });
```

---

## Migraciones con Drizzle

Para definir y ejecutar migraciones del esquema SQL:

### 1. Definir el esquema en TypeScript

```typescript
import { sqliteTable, text, integer, real } from 'drizzle-orm/sqlite-core';

export const customers = sqliteTable('customers', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  email: text('email'),
  createdAt: integer('created_at', { mode: 'timestamp' }),
});
```

### 2. Generar migración

```bash
pnpm drizzle-kit generate
```

### 3. Aplicar migración desde Rust

Las migraciones se aplican al iniciar el backend mediante `turso_core`, que ejecuta los archivos SQL generados por Drizzle Kit.

---

## CDC Sync (Change Data Capture)

El motor de CDC captura cada mutación en la base SQL y la propaga a los peers conectados vía libp2p:

1. **Captura:** `turso_core` escribe cada cambio en una tabla `_cdc_log`.
2. **Propagación:** El worker de CDC lee el log, serializa los cambios como eventos P2P y los publica en el namespace correspondiente.
3. **Replicación:** Cada peer remoto recibe el evento, valida el HLC y aplica el cambio en su instancia local de SQL.
4. **Live Queries:** Cuando un peer recibe un cambio de CDC, el motor de live queries re-evalúa las suscripciones activas cuyas dependencias coincidan y emite los resultados actualizados al frontend.

---

## Estructura de Crates

```
crates/
├── syntrix-core/   ← P2P auth, sync, heartbeats, CDC, live queries
└── turso_core/     ← Motor SQL embebido (fork de Limbo)
```
