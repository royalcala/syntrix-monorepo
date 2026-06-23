# Syntrix — Plan Maestro

## Documentos de diseño

| Doc | Contenido | Estado |
|-----|-----------|--------|
| [`03-arquitectura.md`](./03-arquitectura.md) | Stack: iroh-docs, 4 namespaces. Decisión con consenso de 4 IAs. | 🟡 Histórico |
| [`04-workspace.md`](./04-workspace.md) | UI: entity-based grid + detail panel, TanStack Table + shadcn, FieldRegistry. | 🟡 Base |
| [`05-vision.md`](./05-vision.md) | La matriz completa: 100+ espacios operativos, plugins, workflows. | 🟡 Visión |
| [`06-correcciones.md`](./06-correcciones.md) | Correcciones unánimes de 6 IAs. | 🟡 Histórico |
| [`08-plan-ui.md`](./08-plan-ui.md) | Plan UI detallado: 11 fases de componentes. | 🟢 Activo |
| [`09-arquitectura-final.md`](./09-arquitectura-final.md) | Iroh KV Relational Engine + Tantivy para búsqueda full-text. | 🟢 Activo |

---

## Fase 1: Workspace UI — ✅ Completada

**Objetivo:** Reemplazar las pantallas actuales del client por el sistema de entidades con grid + detail panel.

### Semana 1: Core Workspace Engine ✅

| Tarea | Estado | Descripción |
|-------|--------|-------------|
| 1.1 `FieldRegistry` | ✅ | 8 field types base implementados |
| 1.2 `EntityGrid` | ✅ | TanStack Table + React Query |
| 1.3 `DetailPanel` | ✅ | Panel lateral con tabs, edit/create |
| 1.4 Relations pre-compute | ⚠️ | Pendiente (HashMap O(1) fuera del render loop) |
| 1.5 Grid deps | ✅ | `@tanstack/react-table` + `@tanstack/react-query` |

### Semana 2: Entidades MVP ✅

| Tarea | Estado | Descripción |
|-------|--------|-------------|
| 2.1 `customersEntity` | ✅ | Grid + detail panel |
| 2.2 `invoicesEntity` | ✅ | Grid + detail panel |
| 2.3 `productsEntity` | ✅ | Grid + detail panel |
| 2.4 `ordersEntity` | ✅ | Grid + detail panel |
| 2.5 Reactividad CRUD | ✅ | `setQueryData` + `refetchQueries` |

### Semana 3: Navegación y UX ✅

| Tarea | Estado | Descripción |
|-------|--------|-------------|
| 3.1 URL routing | ✅ | `/customers`, `/invoices`, etc. |
| 3.2 Sidebar navigation | ✅ | AppShell con sección org + device |
| 3.3 Toolbar | ✅ | Vistas, botón +Nuevo, contador |
| 3.4 Empty states | ✅ | Mensajes y CTA cuando no hay datos |
| 3.5 Error boundaries | ⚠️ | Parcial |

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

| Feature | Prioridad | Estado | Notas |
|---------|-----------|--------|-------|
| Workflow de factura | Alta | ❌ | draft → open → paid → cancelled |
| Dashboard básico | Alta | ❌ | KPIs: ventas del mes, facturas pendientes |
| Importar datos (CSV/Excel) | Alta | ❌ | Crítico para onboarding |
| Exportar CSV/Excel | Media | ❌ | Desde el grid con columnas visibles |
| **Búsqueda global (Ctrl+K)** | **Alta** | ⚠️ | **Motor: Tantivy (Rust embebido). Reemplaza FlexSearch.** |
| Adjuntar archivos | Media | ❌ | PDFs a facturas, imágenes a productos |
| Números de factura | Alta | ❌ | Formato `A-0042-XA1` |
| Notificaciones in-app | Alta | ❌ | Toast cuando otro peer modifica un registro |

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

| Prioridad | Qué | Por qué | Estado |
|-----------|-----|---------|--------|
| 🔴 Crítica | El grid scrollea a 60 FPS con 10,000 filas | "El grid es el producto" | ❌ (falta virtualización) |
| 🔴 Crítica | Permisos por namespace bloquean payroll de sales | Sin esto, el producto es invendible | ✅ |
| 🔴 Crítica | Eventos granulares por campo, no por fila | LWW sobre fila completa aplasta ediciones concurrentes | ⚠️ |
| 🟡 Alta | Motor de búsqueda Tantivy | Búsqueda fuzzy, BM25 ranking, snippets — reemplaza FlexSearch | 🔜 Próximo |
| 🟡 Alta | `schemaVersion` en cada entry + migraciones | El primer cambio de schema rompe peers offline | ❌ |
| 🟡 Alta | Snapshots periódicos en iroh-docs | Sin snapshots, 6 meses de eventos = startup lento | ❌ |
| 🟡 Alta | `display_id` ≠ `document_key` | El folio es human-readable, la key usa HLC | ❌ |
| 🟢 Media | `commit_batch_events` en Tauri | Evitar cuellos de botella al editar en lote | ❌ |
| 🟢 Media | Tests E2E multi-peer + Vitest unitarios | Confianza en sync P2P y reactividad | ⚠️ (Vitest configurado, tests parciales) |

---

## Definición de "listo para producción"

- [x] Admin crea org con 4 namespaces, agrega dispositivos, comparte tickets
- [x] Client muestra grid de clientes, facturas, productos con datos reales de iroh-docs
- [x] Click en fila del client abre detail panel con tabs funcionales
- [x] Admin detail panel no muestra pestaña de historial (deshabilitado por diseño)
- [x] Editar celda → commit_event → sync a otros peers
- [x] Sales NO recibe payroll (privacidad entre namespaces)
- [ ] Ctrl+K busca clientes y facturas en <100ms **(próximo: Tantivy)**
- [ ] Grid scrollea 10,000 facturas a 60 FPS **(requiere virtualización)**
- [ ] Adapter reconstruye 10,000 eventos desde redb en <3s al arrancar
- [x] Desconectar internet → seguir operando → reconectar → sync automático

---

## Principio rector

> Menos infraestructura, más libertad operativa. El valor para el nodo está en facturas, inventario y reportes — no en si el log se sincroniza por un protocolo perfecto.
