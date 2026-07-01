---
title: "Esquemas y Drizzle ORM"
description: "Definición centralizada de entidades, campos, índices y relaciones mediante Drizzle ORM — source of truth para toda la aplicación."
---

# Esquemas y Drizzle ORM

Syntrix define sus entidades, campos, índices y relaciones mediante **esquemas Drizzle** (TypeScript). Estos esquemas son la **única fuente de verdad** para la generación de tablas SQL, índices, búsqueda FTS, migraciones y exportación JSON.

---

## Arquitectura

Cada aplicación (`apps/*/`) define sus esquemas en un archivo `drizzle/schema.ts`:

```
apps/
├── admin/
│   └── drizzle/
│       └── schema.ts       ← entidades: orgs, members, roles, devices
├── app/
│   └── drizzle/
│       └── schema.ts       ← entidades: customers, products, invoices, orders, payroll
```

Los esquemas se compilan a SQL mediante `just drizzle-gen`, que genera:
- Sentencias `CREATE TABLE` con tipos, constraints y defaults
- `CREATE INDEX` para cada columna marcada con `@index()`
- Tablas virtuales FTS5 para búsqueda full-text
- Relaciones entre entidades

---

## Entidades Registradas

| Entidad | Campos Indexados | Campos Buscables | Relaciones |
|---------|-----------------|------------------|------------|
| customers | name | name, email, phone, rfc, address | — |
| suppliers | name | name, email, phone, rfc, address | — |
| products | name, price, cost, sku, category, stock | name, description | — |
| invoices | folio, customer_id, date, total, status | notes | customer_id → customers.id |
| orders | folio, customer_id, date, total, status | notes | customer_id → customers.id |
| payroll | name, department, salary, role, active | name, payment_method | — |

---

## Permisos y Entidades

Los permisos operan directamente sobre nombres de entidad. Todas las entidades comparten un topic gossip por org:

1. **Resolución directa**: `can_open`/`can_write` contienen nombres de entidad (o `"*"`). Se resuelve con `can_access(lista, entidad)` → `lista.contains(entidad) || lista.contains("*")`.
2. **Validación por evento**: Cada evento recibido via gossip se valida contra `can_write` del rol del autor antes de indexarse.
3. **Validación en queries**: `query_entity()` verifica que el nombre de la entidad esté en `can_open` del rol del dispositivo.

Ver [TopicIds y Autorización](/arquitectura/namespaces/) para más detalles.
