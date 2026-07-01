---
title: "Syntrix — Estado Actual y Documento Maestro (V1.0)"
---

> **Nota:** Este documento consolida y reemplaza a los documentos `07-plan.md`, `08-plan-ui.md` y `09-arquitectura-final.md` como la fuente de verdad viva del proyecto.

Syntrix es un ERP P2P (Peer-to-Peer) local-first construido con Tauri, Rust, React y libp2p. Su objetivo es ofrecer una experiencia ultra-rápida y colaborativa sin depender de servidores centralizados.

---

## 1. Arquitectura Técnica (El Motor)

El sistema funciona con un enfoque donde el frontend es ultra-ligero y delega toda la carga computacional (almacenamiento, indexación, búsqueda) al backend Rust mediante comandos IPC de Tauri.

### 1.1 Capa de Datos (Source of Truth)
- **Tecnología:** `libp2p gossipsub` + `Limbo SQL`
- **Función:** Sincronización P2P multi-dispositivo y almacenamiento inmutable de eventos.
- **Estructura:** Los datos se guardan como payloads JSON inmutables. El control de concurrencia se maneja mediante HLC (Hybrid Logical Clocks) en la llave del evento. Cada entidad tiene su propio namespace P2P para aislar permisos a nivel granular.

### 1.2 Capa SQL (Limbo)
- **Tecnología:** `turso_core` (fork de Limbo, basado en SQLite)
- **Función:** Proveer consultas SQL eficientes con índices, joins y FTS.
- **Schema-Driven:** Las tablas se generan desde el Registry de Esquemas, con columnas tipadas, índices y constraints.
- **Migraciones:** Drizzle Kit genera los archivos SQL de migración; `turso_core` los aplica al iniciar.
- **CDC Sync:** Cada mutación se captura en `_cdc_log` y se replica a los peers vía libp2p.

### 1.3 FTS Nativo (Limbo)
- **Tecnología:** FTS5 embebido en Limbo / SQLite
- **Función:** Búsqueda full-text con ranking BM25, fuzzy search y snippets.
- **Schema-Driven:** Solo indexa columnas declaradas como `#[searchable]` en el Registry de Esquemas.
- **Integración:** El motor FTS se actualiza automáticamente desde CDC, usando los cambios del log para re-indexar únicamente las columnas relevantes. El frontend consume esto mediante `search_entity` por IPC para búsqueda en grid y para búsqueda global en el Command Palette (Ctrl+K) con snippets resaltados en HTML.

---

## 2. Arquitectura de UI (Frontend)

### 2.1 Stack Core
- **Framework:** React + Vite + TypeScript
- **Capa de Transporte:** `@tanstack/react-query`
- **Tablas y UI:** `@tanstack/react-table` + `shadcn/ui` + `Tailwind CSS`

### 2.2 Patrones de Diseño UI
- **FieldRegistry:** Un registro central (`fields.ts`) que define cómo se renderiza, formatea y valida cada tipo de dato (text, number, currency, status, relation, etc.) tanto en el Grid como en el DetailPanel.
- **Reactividad CRUD (Optimistic Updates):** Al guardar un cambio, el frontend muta la caché local inmediatamente (`queryClient.setQueryData()`) y luego confirma (`queryClient.refetchQueries()`) tras invocar `commit_event` a Rust.
- **Layout Unificado:** Todas las entidades (clientes, facturas, productos) usan el mismo componente `EntityGrid` y `DetailPanel`. La creación y edición ocurren exclusivamente dentro del DetailPanel.

---

## 3. Estado de Desarrollo (Roadmap Unificado)

### ✅ Completado y en Producción
- **Infraestructura Core:** Admin crea organización con namespaces por entidad, agrega dispositivos y comparte tickets selectivos por rol.
- **Registry de Esquemas:** Definición centralizada de entidades, campos, tipos, índices, relaciones y versiones. Exportable a JSON para frontend.
- **Motor SQL Schema-Driven:** Generación de tablas SQL con columnas tipadas, índices sortables y FTS desde el Registry.
- **FTS Nativo (Limbo):** Indexación automática de columnas `#[searchable]` mediante FTS5, con BM25, fuzzy search y snippets.
- **Upcasters y Migraciones:** Eventos con `schema_version`, cadena de upcasters deterministas. Ejemplo: CustomerV1ToV2 (address string → struct).
- **CDC Sync + Live Queries:** Captura de cambios en `_cdc_log`, replicación P2P, y suscripciones SQL en vivo desde el frontend.
- **Auditoría:** Comando `audit_query` que lee directo del log de eventos P2P con filtros por entidad, tipo, nodo y rango de tiempo.
- **Tauri Bridge:** Comandos `query_entity`, `query_entity_advanced`, `commit_event`, `audit_query`, `get_schema_registry`, `live_subscribe`, `live_unsubscribe` funcionales.
- **Permisos P2P:** Aislamiento real entre roles (Sales no lee Payroll).
- **Workspace UI Core:** `EntityGrid` y `DetailPanel` renderizando dinámicamente según la entidad.
- **Reactividad de UI:** Edición de celdas/formularios actualiza la UI y sincroniza a otros peers.
- **Field Types Base:** Text, number, currency, date, select, status, boolean implementados en Grid y Forms.
- **Motor de Búsqueda Full-Text:** Búsqueda difusa, BM25 y snippets en Rust, indexación automática mediante CDC, y comando `search_entity` expuesto.
- **Búsqueda Dinámica en Grids:** Filtro del `EntityGrid` conectado al motor FTS vía debounce + IPC.
- **Búsqueda Global y Command Palette (Ctrl+K):** Cross-entity search en `App.tsx` conectado al motor FTS, mostrando snippets de coincidencia.
- **Navegación e Interacción Integrada (Deep Linking):** Seleccionar un resultado del Command Palette redirige a la vista de la entidad, selecciona la fila y abre el panel de detalles automáticamente usando `useSearchParams`.
- **Detail Panel Responsivo:** Bottom sheet para móviles y panel lateral para desktop implementado.
- **Explorador de Esquemas (`/schemas`):** Vista en Admin Console de entidades, campos, índices y relaciones desde el Registry.
- **Auditoría de Eventos (`/audit`):** Feed cronológico con filtros en Admin Console.

### ⚠️ En Progreso / Parcial
- **Detail Panel Avanzado:** Falta resize handle y sub-grids funcionales (ej. Ver facturas dentro del cliente).
- **Grid Avanzado:** Sort funcional. Filtros parciales (solo globales). Faltan columnas redimensionables/ocultables y multi-selección.
- **Estados Visuales y Transiciones:** Implementados Toasts y Skeletons. Faltan transiciones suaves en paneles.
- **Feedback Visual de Sincronización:** Falta construir indicadores UI en la barra lateral (`SyncStatusIndicator`) mostrando estado P2P real (online/offline, docs pending).

### 🔜 Próximas Prioridades Técnicas (El Backlog Inmediato)
1. **Persistencia de Membresía e Identidades P2P:** Reemplazar `MemStore` y claves efímeras en `identity.rs` por almacenamiento persistente para que la membresía y sync sobreviva a reinicios.
2. **Sistema de Presencia (Heartbeat):** Implementar protocolo de latido escribiendo un timestamp en `control_doc` cada 30s para identificar clientes offline y online con exactitud real, con vista en el Admin Dashboard de Sync.
3. **Virtualización del Grid:** Implementar TanStack Virtual en `EntityGrid` para scroll a 60 FPS con miles de filas.
4. **Push Events de Reactividad:** Reemplazar el `refetchQueries` manual por un listener global (`emit("entity_changed")` desde Rust) para reaccionar a cambios hechos por *otros* peers en tiempo real.
5. **Codegen Zod desde Registry:** Generar tipos TypeScript y esquemas Zod automáticamente desde `get_schema_registry()`.

### 🛠️ Pendiente (Herramientas de Consola Admin)
- **Data Explorer (Visor JSON Crudo):** Vista especializada para diagnosticar la base de datos P2P. Muestra metadatos puros de la red P2P (Doc Hash, HLC, Autor) y el JSON crudo. Permite identificar datos corruptos, visualizar estado local de la base SQL y emitir eventos correctivos a la red.

### ❌ Pendiente (Features de Negocio)
- Workflow de facturas (draft → open → paid).
- Importación/Exportación de CSV.
- Dashboard de KPIs.
- Números de folio auto-generados (`A-0042-XA1`).
- Adjuntar archivos (PDFs/Imágenes) vía P2P transferencia.
- Vistas Guardadas (Filtros y orden predefinidos persistidos).

---

## 4. Definición de MVP (Listo para Lanzamiento)
- Motor FTS nativo implementado y Command Palette funcional.
- Grid scrollea 10,000 registros a 60 FPS (Virtualización).
- Sincronización multi-peer totalmente reactiva.
- Recuperación offline probada y robusta.
- Workflows básicos de facturación operando.
