# Syntrix — Estado Actual y Documento Maestro (V1.0)

> **Nota:** Este documento consolida y reemplaza a los documentos `07-plan.md`, `08-plan-ui.md` y `09-arquitectura-final.md` como la fuente de verdad viva del proyecto.

Syntrix es un ERP P2P (Peer-to-Peer) local-first construido con Tauri, Rust, React e iroh-docs. Su objetivo es ofrecer una experiencia ultra-rápida y colaborativa sin depender de servidores centralizados.

---

## 1. Arquitectura Técnica (El Motor)

El sistema funciona con un enfoque donde el frontend es ultra-ligero y delega toda la carga computacional (almacenamiento, indexación, búsqueda) al backend Rust mediante comandos IPC de Tauri.

### 1.1 Capa de Datos (Source of Truth)
- **Tecnología:** `iroh-docs`
- **Función:** Sincronización P2P multi-dispositivo y almacenamiento inmutable de eventos.
- **Estructura:** Los datos se guardan como payloads JSON inmutables. El control de concurrencia se maneja mediante HLC (Hybrid Logical Clocks) en la llave del evento. Se utilizan diferentes namespaces para aislar permisos (ej. ventas vs nómina).

### 1.2 Capa Relacional (Local Engine)
- **Tecnología:** `redb` (Embebido en Rust)
- **Función:** Proveer consultas eficientes y lookups O(1).
- **Cómo funciona:** `indexes.rs` actúa como un indexador en segundo plano. Al recibir un evento de iroh, extrae el JSON y crea índices secundarios vacíos en redb (`idx:{org}:{entity}:{field}:{value}:{doc_id}`) para escaneos de prefijo ultra-rápidos.

### 1.3 Capa de Búsqueda (Completada e Integrada)
- **Tecnología:** `Tantivy` (Motor Full-Text embebido en Rust)
- **Función:** Búsqueda difusa (fuzzy search), BM25 ranking y snippets.
- **Cómo funciona:** Índice paralelo a redb. Se actualiza automáticamente desde `upsert_document` después de realizar la confirmación transaccional. El frontend consume esto mediante la query reactiva que invoca a `search_entity` por IPC para búsqueda en el grid y para búsqueda global en el Command Palette (Ctrl+K) con snippets resaltados en HTML.

---

## 2. Arquitectura de UI (Frontend)

### 2.1 Stack Core
- **Framework:** React + Vite + TypeScript
- **Capa de Transporte:** `@tanstack/react-query` (Reemplazó a TanStack DB).
- **Tablas y UI:** `@tanstack/react-table` + `shadcn/ui` + `Tailwind CSS`.

### 2.2 Patrones de Diseño UI
- **FieldRegistry:** Un registro central (`fields.ts`) que define cómo se renderiza, formatea y valida cada tipo de dato (text, number, currency, status, relation, etc.) tanto en el Grid como en el DetailPanel.
- **Reactividad CRUD (Optimistic Updates):** Al guardar un cambio, el frontend muta la caché local inmediatamente (`queryClient.setQueryData()`) y luego confirma (`queryClient.refetchQueries()`) tras invocar `commit_event` a Rust.
- **Layout Unificado:** Todas las entidades (clientes, facturas, productos) usan el mismo componente `EntityGrid` y `DetailPanel`. La creación y edición ocurren exclusivamente dentro del DetailPanel.

---

## 3. Estado de Desarrollo (Roadmap Unificado)

### ✅ Completado y en Producción
- **Infraestructura Core:** Admin crea organización con 4 namespaces, agrega dispositivos y comparte tickets selectivos por rol.
- **Motor Relacional:** `indexes.rs` generando índices en redb.
- **Tauri Bridge:** Comandos `query_entity` y `commit_event` funcionales.
- **Permisos P2P:** Aislamiento real entre roles (Sales no lee Payroll).
- **Workspace UI Core:** `EntityGrid` y `DetailPanel` renderizando dinámicamente según la entidad.
- **Reactividad de UI:** Edición de celdas/formularios actualiza la UI y sincroniza a otros peers.
- **Field Types Base:** Text, number, currency, date, select, status, boolean implementados en Grid y Forms.
- **Motor de Búsqueda Híbrido Tantivy + redb:** Búsqueda difusa, BM25 y snippets en Rust (`search.rs`), indexación automática en writes, y comando `search_entity` expuesto.
- **Búsqueda Dinámica en Grids:** Filtro del `EntityGrid` conectado a Tantivy vía debounce + IPC.
- **Búsqueda Global y Command Palette (Ctrl+K):** Cross-entity search en `App.tsx` conectado a Tantivy, mostrando snippets de coincidencia.
- **Navegación e Interacción Integrada (Deep Linking):** Seleccionar un resultado del Command Palette redirige a la vista de la entidad, selecciona la fila y abre el panel de detalles automáticamente usando `useSearchParams`.
- **Detail Panel Responsivo:** Bottom sheet para móviles y panel lateral para desktop implementado.

### ⚠️ En Progreso / Parcial
- **Detail Panel Avanzado:** Falta resize handle y sub-grids funcionales (ej. Ver facturas dentro del cliente).
- **Grid Avanzado:** Sort funcional. Filtros parciales (solo globales). Faltan columnas redimensionables/ocultables y multi-selección.
- **Estados Visuales y Transiciones:** Implementados Toasts y Skeletons. Faltan transiciones suaves en paneles.

### 🔜 Próximas Prioridades Técnicas (El Backlog Inmediato)
1. **Paginación Server-Side:** Soporte para `limit` y `offset` en `query_entity` para no colapsar la RAM al tener >10k registros.
2. **Virtualización del Grid:** Implementar TanStack Virtual en `EntityGrid` para scroll a 60 FPS con miles de filas.
3. **Push Events de Reactividad:** Reemplazar el `refetchQueries` manual por un listener global (`emit("entity_changed")` desde Rust) para reaccionar a cambios hechos por *otros* peers en tiempo real.

### ❌ Pendiente (Features de Negocio)
- Workflow de facturas (draft → open → paid).
- Importación/Exportación de CSV.
- Dashboard de KPIs.
- Números de folio auto-generados (`A-0042-XA1`).
- Adjuntar archivos (PDFs/Imágenes) vía iroh blobs.
- Vistas Guardadas (Filtros y orden predefinidos persistidos).

---

## 4. Definición de MVP (Listo para Lanzamiento)
- Motor Tantivy implementado y Command Palette funcional.
- Grid scrollea 10,000 registros a 60 FPS (Virtualización).
- Sincronización multi-peer totalmente reactiva.
- Recuperación offline probada y robusta.
- Workflows básicos de facturación operando.

