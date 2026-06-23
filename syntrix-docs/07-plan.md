# Syntrix — Plan Maestro

## Documentos de diseño

| Doc | Contenido |
|-----|-----------|
| [`03-arquitectura.md`](./03-arquitectura.md) | Stack: iroh-docs, 4 namespaces, adapter TanStack DB. Decisión final con consenso de 4 IAs. |
| [`04-workspace.md`](./04-workspace.md) | UI: entity-based grid + detail panel, TanStack Table + shadcn, FieldRegistry. |
| [`05-vision.md`](./05-vision.md) | La matriz completa: 100+ espacios operativos, plugins, workflows, inteligencia, multi-idioma. |
| [`06-correcciones.md`](./06-correcciones.md) | Índices en TanStack DB (no redb), correcciones unánimes de 6 IAs. |
| [`09-arquitectura-final.md`](./09-arquitectura-final.md) | **NUEVO:** Reemplazo de TanStack DB por Iroh KV Relational Engine (Cero migraciones, RAM optimizada). |

---

## Fase 1: Workspace UI (2-3 semanas)

**Objetivo:** Reemplazar las pantallas actuales del client por el sistema de entidades con grid + detail panel.

### Semana 1: Core Workspace Engine

| Tarea | Descripción | Docs ref |
|-------|-------------|----------|
| 1.1 `FieldRegistry` | Implementar registry con 8 field types base | [`04-workspace.md`](./04-workspace.md) |
| 1.2 `EntityGrid` | Wrapper genérico de TanStack Table + TanStack DB | [`04-workspace.md`](./04-workspace.md) |
| 1.3 `DetailPanel` | Panel lateral con tabs | [`04-workspace.md`](./04-workspace.md) |
| 1.4 Relations pre-compute | HashMap O(1) fuera del render loop | [`06-correcciones.md`](./06-correcciones.md) |
| 1.5 Install grid deps | `@tanstack/react-table@^8.21.2` | [`04-workspace.md`](./04-workspace.md) |

### Semana 2: Entidades MVP

| Tarea | Descripción |
|-------|------------|
| 2.1 `customersEntity` | Grid de clientes + detail panel con tabs |
| 2.2 `invoicesEntity` | Grid de facturas + detail con tab Items |
| 2.3 `productsEntity` | Grid de productos + detail |
| 2.4 `ordersEntity` | Grid de órdenes + detail |
| 2.5 Optimistic updates | onCellEdited → commit_event → rollback si falla |

### Semana 3: Navegación y UX

| Tarea | Descripción |
|-------|------------|
| 3.1 URL routing | `/clients`, `/clients/:id`, `/invoices`, `/invoices/:id` |
| 3.2 Sidebar navigation | Sidebar con entidades + favoritos + búsqueda |
| 3.3 Toolbar | Vistas guardadas, filtros rápidos, botón +Nuevo |
| 3.4 Empty states | Mensajes y acciones cuando no hay datos |
| 3.5 Error boundaries | Por entidad, un plugin roto no tumbar el ERP |

---

## Fase 2: Admin UI (1-2 semanas)

**Objetivo:** Migrar el admin app al mismo sistema de entidades.

| Tarea | Descripción |
|-------|------------|
| Admin 1 | Copiar Workspace Engine (FieldRegistry, EntityGrid, DetailPanel) |
| Admin 2 | Entity: `devicesEntity` — grid de dispositivos con status, rol, persona |
| Admin 3 | Entity: `rolesEntity` — grid de roles con can_open/can_write editables |
| Admin 4 | Entity: `orgsEntity` — grid de organizaciones |
| Admin 5 | Flujo: crear org → 4 namespaces + admin device (ya implementado en Rust) |
| Admin 6 | Flujo: invitar dispositivo → QR/link con tickets selectivos por rol |
| Admin 7 | Flujo: revocar dispositivo → marcar active: false en control doc |

---

## Fase 3: Features de negocio (en curso)

**Objetivo:** Agregar features que los usuarios necesitan para operar.

| Feature | Prioridad | Notas |
|---------|-----------|-------|
| Workflow de factura | Alta | draft → open → paid → cancelled. Transiciones con validaciones. |
| Dashboard básico | Alta | KPIs: ventas del mes, facturas pendientes, top clientes. |
| Importar datos (CSV/Excel) | Alta | Crítico para onboarding de clientes (clientes, productos). |
| Exportar CSV/Excel | Media | Desde el grid, con las columnas visibles. |
| Búsqueda global (Ctrl+K) | Media | Índice FlexSearch en memoria. Clientes, facturas, acciones. |
| Adjuntar archivos | Media | PDFs a facturas, imágenes a productos. StreamFS en Fase 4? |
| Números de factura | Alta | Formato `A-0042-XA1`. Ya decidido. |
| Notificaciones in-app (Sync UI) | Alta | Toast cuando otro peer modifica tu registro o si hay un conflicto LWW. |

---

## Fase 4: Escala y enterprise (cuando haya 5+ clientes)

**Objetivo:** Features que solo se necesitan con volumen real.

| Feature | Gatillo |
|---------|---------|
| Plugin system | Primer cliente que pide SAT CFDI |
| Workflow engine completo | Múltiples entidades con workflows |
| Report builder visual | Clientes piden reportes personalizados |
| Multi-idioma (i18n) | Primer cliente fuera de México |
| Kanban, Calendar, Gantt | Entidades que no son tablas (proyectos, entregas) |
| Field-level permissions | Cliente enterprise con datos sensibles por campo |
| Apache Iggy (backup + BI) | 5+ sucursales activas |
| Web client (sin Tauri) | Demanda de acceso desde browser sin instalar |
| Mobile (Tauri mobile) | Demanda de iOS/Android |

---

## Lo que NO está en el plan (hasta que un cliente lo pida)

- Facturación electrónica SAT (CFDI) — plugin en Fase 4
- Multi-moneda / multi-país — field types nuevos cuando se necesiten
- Punto de venta (POS) — escape hatch, entidad con override.fullPage
- Conciliación bancaria — plugin en Fase 4
- Portal de clientes — web client en Fase 4

---

## Prioridades técnicas continuas

| Prioridad | Qué | Por qué |
|-----------|-----|---------|
| 🔴 Crítica | El grid scrollea a 60 FPS con 10,000 filas | "El grid es el producto" |
| 🔴 Crítica | Permisos por namespace bloquean payroll de sales | Sin esto, el producto es invendible |
| 🔴 Crítica | Eventos granulares por campo, no por fila | LWW sobre fila completa aplasta ediciones concurrentes de campos distintos |
| 🟡 Alta | `schemaVersion` en cada entry + migraciones en adapter | El primer cambio de schema rompe peers offline sin esto |
| 🟡 Alta | Snapshots periódicos en iroh-docs | Sin snapshots, 6 meses de eventos = startup lento |
| 🟡 Alta | Adapter con buffer/batch para reconexión offline | 5,000+ entries de golpe saturan TanStack DB |
| 🟡 Alta | Manejo de memoria en TanStack DB (Límites/Query params) | Evitar OOM (Out Of Memory) en el navegador al cargar 100k registros |
| 🟡 Alta | `display_id` ≠ `document_key` (HLC en key, no en folio) | El folio es human-readable, la key usa HLC para orden causal |
| 🟢 Media | `commit_batch_events` en el adapter | Evitar cuellos de botella en el bridge de Tauri al editar registros en lote |
| 🟢 Media | Tests E2E multi-peer (3 nodos iroh) | Confianza en sync P2P |

---

## Definición de "listo para producción"

- [x] Admin crea org con 4 namespaces, agrega dispositivos, comparte tickets
- [ ] Client muestra grid de clientes, facturas, productos con datos reales de iroh-docs
- [ ] Click en fila del client abre detail panel con tabs funcionales (datos, historial cargado bajo demanda de iroh-docs)
- [ ] Admin detail panel no muestra la pestaña de historial (deshabilitado por diseño para el MVP)
- [ ] Editar celda → commit_event → sync a otros peers
- [ ] Sales NO recibe payroll (privacidad entre namespaces)
- [ ] Ctrl+K busca clientes y facturas en <100ms
- [ ] Grid scrollea 10,000 facturas a 60 FPS
- [ ] Adapter reconstruye 10,000 eventos desde redb a TanStack DB en <3s al arrancar (sin bloquear UI)
- [ ] Desconectar internet → seguir operando → reconectar → sync automático

---

## Principio rector

> Menos infraestructura, más libertad operativa. El valor para el nodo está en facturas, inventario y reportes — no en si el log se sincroniza por un protocolo perfecto.
