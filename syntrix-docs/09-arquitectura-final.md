# Syntrix — Arquitectura Final (Iroh KV Relational Engine)

Este documento define la arquitectura definitiva a largo plazo para Syntrix, reemplazando el uso de bases de datos in-memory (TanStack DB) o relacionales (SQLite/PGLite) en el frontend, por un motor de consultas nativo construido sobre el almacenamiento Key-Value (KV) de `iroh-docs`.

## 1. El Problema a Resolver

- **TanStack DB:** Consumo masivo de memoria RAM al escalar a cientos de miles de registros (todo vive en el JS Heap).
- **SQLite / PGLite:** Requieren migraciones SQL estrictas (`ALTER TABLE`), lo cual destruye la agilidad de desarrollo y es peligroso en entornos locales P2P (corrupción de BD al actualizar versiones). Además, duplica el almacenamiento local que ya maneja `iroh-docs`.

## 2. La Arquitectura Ideal (Sin Compromisos)

El diseño se basa en patrones de bases de datos distribuidas (como FoundationDB o SurrealDB interno), utilizando índices secundarios sobre un almacenamiento Key-Value puro.

### A. Capa de Almacenamiento Única (Rust + `iroh-docs` + `redb`)
`iroh-docs` es la única fuente de verdad. No hay duplicación de datos. Todo documento se almacena como un payload JSON inmutable bajo una llave estructurada en `redb`.
* **Cero migraciones:** El esquema es dinámico (JSON). Si se agrega un campo a las facturas, el frontend lo maneja (ej. asignando valores por defecto o usando Zod).

### B. Indexador Relacional en Segundo Plano (Rust)
Dado que un KV store no puede filtrarse eficientemente, construimos índices secundarios de forma transparente:
1. Al recibir un evento en `iroh` (sincronizado o local), Rust parsea el JSON.
2. Basado en un registro de configuración (ej. "indexar facturas por cliente y estado"), Rust escribe llaves vacías en el KV que actúan como punteros.
3. **Ejemplo de llave de índice:** `idx:invoices:customer_id:<id>:status:<estado> -> invoice_id`.
4. El disco (`redb`) mantiene esto organizado en árboles B (B-Trees), permitiendo escaneos de prefijo ultrarrápidos (microsegundos).

### C. Motor de Consultas en Tauri (Rust)
El frontend ya no procesa toda la colección. En su lugar, delega la carga computacional a Rust.
* Se expone un comando genérico: `invoke("query_entity", { entity: "invoices", where: { status: "paid" }, sort: "date", limit: 50, offset: 0 })`.
* Rust usa los índices precalculados para buscar eficientemente, hidrata los IDs obtenidos buscando los JSONs originales en `iroh-docs`, y devuelve solo la página requerida al frontend.

### D. Frontend Ultra-Ligero (React + TanStack Query)
El adaptador cambia radicalmente:
* Se reemplaza `@tanstack/react-db` por `@tanstack/react-query`.
* **Memoria mínima:** Solo se mantienen en el DOM y en la caché de React Query los 50-100 registros visibles.
* **Reactividad:** Cuando un evento entra a `iroh`, Rust emite un evento global por Tauri (ej. `emit("entity_changed", { entity: "invoices" })`). TanStack Query escucha este evento, invalida su caché (`queryClient.invalidateQueries(["invoices"])`), y automáticamente recarga la tabla.

---

## 3. Beneficios de este Diseño

| Característica | Resultado |
|----------------|-----------|
| **Migraciones de Base de Datos** | Inexistentes. Es 100% JSON-first. |
| **Uso de Memoria (RAM)** | Mínimo en el navegador (solo la página actual). |
| **Velocidad de Query** | Nativa en Rust leyendo de B-Trees en disco sólido. |
| **Escalabilidad** | Soporta millones de registros por tabla sin degradar el UI. |
| **Complejidad del Sync** | Centralizada. Todo sigue siendo el protocolo P2P nativo de iroh. |

---

## 4. Plan de Implementación (Próximos Pasos)

Para pivotar hacia esta arquitectura sin detener el desarrollo actual, el roadmap técnico es:

1. **Rust (Backend):**
   - [ ] Implementar un módulo de índices secundarios simples (`indexes.rs`) que escuche los eventos de `iroh-docs` e inserte llaves en un `redb` local.
   - [ ] Crear el comando `query_entity` en Tauri que lea estos índices.
2. **TypeScript (Frontend):**
   - [ ] Instalar `@tanstack/react-query`.
   - [ ] Refactorizar `EntityGrid.tsx` para usar `useQuery` en lugar de `useLiveQuery`.
   - [ ] Implementar la paginación a nivel de tabla (Virtualización o botones de página cargando por bloques desde Rust).
3. **Limpieza:**
   - [ ] Remover `@tanstack/react-db` y el adapter actual en memoria.
