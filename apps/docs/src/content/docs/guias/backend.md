---
title: "Desarrollo Backend (Rust & Tauri)"
description: "Guía de desarrollo para el core de Rust y comandos en Tauri de Syntrix."
---

# Desarrollo Backend

El núcleo de Syntrix está construido en **Rust**, el cual provee la seguridad de memoria, concurrencia y velocidad requeridas para el motor P2P descentralizado.

## Tecnologías Utilizadas

- **Rust (edición 2021)**: Lenguaje del core.
- **Tauri v2**: Framework para conectar el backend de Rust con la UI web.
- **Iroh**: Protocolo P2P y motor de base de datos relacional de documentos (Iroh KV).
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
