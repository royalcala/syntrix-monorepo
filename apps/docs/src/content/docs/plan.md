---
title: "Hoja de Ruta del Producto"
description: "Ruta estratégica de Syntrix — desde el estado actual hacia el producto a largo plazo."
---

# Hoja de Ruta del Producto

Esta hoja de ruta refleja el estado real del producto y la dirección estratégica a largo plazo. No es un MVP — es un producto sostenible que crece orgánicamente.

Para el detalle técnico de lo que está implementado, consulta [Estado Actual](./estado-actual/).

---

## Principio Rector

> Menos infraestructura, más libertad operativa. El valor para el nodo está en facturas, inventario y reportes — no en si el log se sincroniza por un protocolo perfecto.

Cada feature debe responder a una pregunta: **¿esto le da al usuario más control sobre su operación?** Si no, se pospone.

---

## Fase 0: Fundaciones (✅ Completada)

La arquitectura base está construida y probada.

| Componente | Estado | Descripción |
|-----------|--------|-------------|
| P2P Sync (libp2p) | ✅ | Sync P2P con HLC, validación criptográfica de eventos |
| Motor Relacional (redb) | ✅ | DOCUMENTS + INDEXES + COMPOSITE, schema-driven, codificación sortable |
| Búsqueda Full-Text (Tantivy) | ✅ | BM25, fuzzy search, snippets, integrado al indexador |
| Registry de Esquemas | ✅ | `syntrix-schema` crate, entidades, campos, índices, relaciones, namespaces |
| Migraciones (Upcasters) | ✅ | `schema_version` en eventos, cadena de upcasters deterministas |
| Permisos Schema-Driven | ✅ | `can_open`/`can_write` a nivel entidad, enforcement en queries y commits |
| Workspace UI | ✅ | EntityGrid + DetailPanel, 8 field types, CRUD reactivo |
| Admin Console | ✅ | Gestión de dispositivos, roles (matriz de permisos), orgs, esquemas, auditoría |
| Auditoría de Eventos | ✅ | Feed cronológico del log de eventos con filtros |
| Command Palette (Ctrl+K) | ✅ | Búsqueda global cross-entity con Tantivy |

---

## Fase 1: Operatividad Real (En curso)

**Objetivo:** Hacer que el producto sea usable para operar un negocio real día a día.

| Feature | Prioridad | Estado | Valor |
|---------|-----------|--------|-------|
| Importar datos (CSV/Excel) | 🔴 Crítica | ❌ | Sin esto, onboarding es manual y lento |
| Workflow de factura | 🔴 Crítica | ❌ | draft → open → paid → cancelled con side effects |
| Dashboard básico | 🔴 Crítica | ❌ | KPIs: ventas del mes, facturas pendientes, inventario bajo |
| Folios sin colisión | 🔴 Crítica | ❌ | Formato `{branch}-{counter}-{node}` para facturas offline |
| Exportar CSV/Excel | 🟡 Alta | ❌ | Desde el grid con columnas visibles |
| Virtualización del grid | 🟡 Alta | ❌ | TanStack Virtual para 60 FPS con +10K filas |
| Reactividad push entre peers | 🟡 Alta | ❌ | `emit("entity_changed")` para tiempo real sin polling |
| Codegen Zod desde registry | 🟡 Alta | ❌ | Generar tipos TS desde `get_schema_registry()` |
| Persistencia de identidad P2P | 🟡 Alta | ❌ | Sobrevivir reinicios sin perder membresía/sync |
| Sistema de presencia | 🟢 Media | ❌ | Heartbeats cada 30s, vista online/offline en admin |

---

## Fase 2: Madurez Operativa

**Objetivo:** El producto cubre el ciclo completo de un negocio pequeño/mediano.

| Feature | Gatillo | Valor |
|---------|---------|-------|
| Reportes configurables | Primer cliente que pide reportes beyond dashboard | Builder visual: entidad base → joins → filtros → agrupación → exportar |
| Adjuntar archivos | Primer cliente con facturas PDF o imágenes de producto | P2P transferencia de archivos |
| Multi-moneda | Primer cliente con operaciones internacionales | Field type `currency` con tipo de cambio |
| Notificaciones in-app | Múltiples peers editando concurrentemente | Toast cuando otro peer modifica un registro visible |
| Snapshots periódicos | 6+ meses de eventos acumulados | Compactar historial, startup < 2s con 10K+ registros |
| Vistas alternativas (Kanban, Calendar) | Entidades que no son naturalmente tablas | Pipeline de ventas, calendario de entregas |
| Field-level permissions | Cliente enterprise con datos sensibles | `cost` visible solo para admin/contador |

---

## Fase 3: Ecosistema

**Objetivo:** Syntrix se convierte en plataforma extensible.

| Feature | Gatillo | Valor |
|---------|---------|-------|
| Plugin system | Primer cliente que necesita SAT CFDI o integración bancaria | Terceros extienden sin tocar core: field types, entidades, workflows |
| Workflow engine | Múltiples entidades con estados complejos | Motor de transiciones con guards, side effects, permisos |
| Multi-idioma (i18n) | Primer cliente fuera de México | Lazy loading de traducciones |
| Web client (sin Tauri) | Demanda de acceso desde browser | HTTP API desde Tauri (Axum), sin servidor central |
| Mobile (Tauri mobile) | Demanda de iOS/Android | Mismo core Rust, UI adaptada |
| Archivo histórico (Iggy/Arweave) | 5+ sucursales con años de datos | Backup + analytics offline, purga de datos locales |
| Portal de clientes | Clientes que quieren auto-servicio | Acceso read-only web para clientes externos |

---

## Lo que NO está en la hoja de ruta (hasta que un cliente lo pida)

- Facturación electrónica SAT (CFDI) — plugin en Fase 3
- Conciliación bancaria — plugin en Fase 3
- Punto de venta (POS) — escape hatch cuando se necesite
- Multi-país (regímenes fiscales) — field types nuevos cuando haya demanda
- Permisos por registro (Row-Level Security) — cuando un cliente enterprise lo exija
- Overrides por dispositivo — complejidad innecesaria, roles cubren el caso

---

## Decisiones de arquitectura que ya no se revertirán

Estas decisiones se tomaron en las fundaciones y definen el producto:

1. **P2P event log como única fuente de verdad** — redb y Tantivy son proyecciones volátiles, reconstruidas desde el log.
2. **Rust como motor de consultas** — el frontend nunca carga colecciones completas; delega a Rust vía IPC.
3. **Registry de esquemas como source of truth** — un solo lugar define entidades, índices, relaciones, namespaces y migraciones.
4. **Permisos puros a nivel entidad** — `can_open`/`can_write` contienen nombres de entidad o `"*"`, nunca namespaces.
5. **Namespaces por entidad** — cada entidad del registry tiene su propio namespace P2P. Aislamiento granular de datos sensibles.
6. **Sin servidor central** — todo es P2P. El "backup" es un nodo pasivo, no un servidor.

---

## Cómo medimos el progreso

El producto avanza cuando un usuario real puede:

- [x] Crear una organización, invitar dispositivos, asignar roles
- [x] Operar clientes, facturas, productos, órdenes con datos reales
- [x] Buscar en < 100ms cualquier registro (Ctrl+K)
- [x] Trabajar offline y sincronizar al reconectar
- [x] Auditar todas las mutaciones del colectivo
- [x] Visualizar esquemas, índices y relaciones
- [ ] Importar datos existentes (CSV/Excel) ← próximo hito
- [ ] Ver un dashboard con KPIs del negocio
- [ ] Gestionar el ciclo de vida de una factura
- [ ] Scrollear 10,000 filas a 60 FPS
