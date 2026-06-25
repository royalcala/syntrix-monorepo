---
title: "Syntrix — Arquitectura Final (Iroh KV Relational Engine)"
---

Este documento define la arquitectura definitiva para Syntrix. El frontend delega **toda** la carga computacional (almacenamiento, indexación, búsqueda) al backend Rust, comunicándose exclusivamente vía comandos Tauri IPC.

> **Estado:** La migración desde TanStack DB está completada. El motor KV relacional (`indexes.rs`) y React Query están en producción.

## 1. El Problema a Resolver

- **TanStack DB:** Consumo masivo de memoria RAM al escalar a cientos de miles de registros (todo vive en el JS Heap).
- **SQLite / PGLite:** Requieren migraciones SQL estrictas (`ALTER TABLE`), lo cual destruye la agilidad de desarrollo y es peligroso en entornos locales P2P (corrupción de BD al actualizar versiones). Además, duplica el almacenamiento local que ya maneja `iroh-docs`.

## 2. La Arquitectura Ideal (Sin Compromisos)

El diseño se basa en patrones de bases de datos distribuidas (como FoundationDB o SurrealDB interno), utilizando índices secundarios sobre un almacenamiento Key-Value puro.

### A. Capa de Almacenamiento Única (Rust + `iroh-docs` + `redb`)
`iroh-docs` es la única fuente de verdad. No hay duplicación de datos. Todo documento se almacena como un payload JSON inmutable bajo una llave estructurada en `redb`.
* **Cero migraciones:** El esquema es dinámico (JSON). Si se agrega un campo a las facturas, el frontend lo maneja (ej. asignando valores por defecto o usando Zod).

### B. Indexador Relacional en Segundo Plano (Rust) — ✅ Implementado
Dado que un KV store no puede filtrarse eficientemente, construimos índices secundarios de forma transparente:
1. Al recibir un evento en `iroh` (sincronizado o local), Rust parsea el JSON.
2. Para cada campo primitivo (string, número, booleano), Rust genera llaves de índice automáticamente.
3. **Formato de llave:** `idx:{org_id}:{entity}:{field}:{value_lowercase}:{doc_id}` → valor vacío.
4. El disco (`redb`) mantiene esto organizado en árboles B (B-Trees), permitiendo escaneos de prefijo ultrarrápidos (microsegundos).

### B.2 Motor de Búsqueda Full-Text (Tantivy) — 🔜 Próximo
Para búsquedas de texto libre (barra "Buscar..." y Ctrl+K), se integra **Tantivy** como índice de búsqueda paralelo a redb:
- Tantivy indexa campos de texto (nombre, email, RFC, etc.) con un índice invertido
- Soporta: **búsqueda difusa** ("Akme" → "Acme"), **ranking BM25**, **snippets con highlights**
- redb sigue siendo la fuente de verdad para datos; Tantivy es solo el índice de búsqueda
- Se alimenta automáticamente desde `upsert_document()` en `indexes.rs`
- Implementación: `search.rs` (~120 LOC), expuesto como comando `search_entity`

### C. Motor de Consultas en Tauri (Rust)
El frontend ya no procesa toda la colección. En su lugar, delega la carga computacional a Rust.
* Se expone un comando genérico: `invoke("query_entity", { entity: "invoices", where: { status: "paid" }, sort: "date", limit: 50, offset: 0 })`.
* Rust usa los índices precalculados para buscar eficientemente, hidrata los IDs obtenidos buscando los JSONs originales en `iroh-docs`, y devuelve solo la página requerida al frontend.

### D. Frontend Ultra-Ligero (React + TanStack Query) — ✅ Implementado
* `@tanstack/react-query` reemplazó a `@tanstack/react-db`.
* **Memoria mínima:** Solo se mantienen en el DOM y en la caché de React Query los registros de la entidad activa.
* **Reactividad post-mutación:** Tras cada `commit_event`, el frontend ejecuta:
  1. `queryClient.setQueryData()` — actualización optimista instantánea en la tabla
  2. `queryClient.refetchQueries()` — confirmación contra la base de datos real (250ms después)
* **Pendiente:** Evento push desde Rust (`emit("entity_changed")`) para reactividad entre peers sin polling.

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

## 4. Plan de Implementación

### Completado ✅
1. **Rust (Backend):**
   - [x] `indexes.rs` — Indexador relacional con índices secundarios automáticos en redb
   - [x] `query_entity` — Comando Tauri para consultar entidades con filtros opcionales
   - [x] `events.rs` — Commit de eventos con HLC, validación de permisos por rol
2. **TypeScript (Frontend):**
   - [x] Migración a `@tanstack/react-query` (EntityGrid.tsx)
   - [x] Reactividad CRUD — `setQueryData` + `refetchQueries` tras mutaciones
   - [x] DetailPanel con inyección de ID para actualizaciones correctas
3. **Limpieza:**
   - [x] `@tanstack/react-db` eliminado del proyecto

### Próximos pasos 🔜
4. **Búsqueda Full-Text (Tantivy):**
   - [ ] `search.rs` — Motor Tantivy embebido (~120 LOC)
   - [ ] Comando `search_entity` con fuzzy search, BM25, snippets
   - [ ] Conectar barra "Buscar..." del EntityGrid a Tantivy
   - [ ] Conectar CommandPalette (Ctrl+K) para búsqueda cross-entity
5. **Escalabilidad:**
   - [ ] Paginación server-side en `query_entity` (limit/offset)
   - [ ] Virtualización con TanStack Virtual para +10K filas
   - [ ] Evento push `emit("entity_changed")` para reactividad entre peers