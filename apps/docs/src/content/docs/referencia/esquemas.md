---
title: "Registry de Esquemas"
description: "Definición centralizada de entidades, campos, índices y relaciones — source of truth para toda la aplicación."
---

# Registry de Esquemas

Syntrix utiliza un **registry de esquemas centralizado** definido en el crate `syntrix-schema` (Rust). Este registry es la **única fuente de verdad** para generación de índices, búsqueda Tantivy, migraciones y exportación JSON.

---

## Arquitectura

```
syntrix-schema/
├── src/
│   ├── lib.rs        # Re-export público
│   ├── schema.rs     # Tipos: EntitySchema, FieldSchema, FieldType, IndexDef, RelationDef, SchemaRegistry
│   ├── entities.rs   # Definiciones de entidades (customers, invoices, etc.)
│   ├── encoded.rs    # Codificación sortable de valores para claves de índice
│   └── upcast.rs     # Trait Upcaster y cadena de migraciones
```

---

## Entidades Registradas

| Entidad | Versión | Campos Indexados | Campos Buscables | Relaciones |
|---------|---------|-----------------|------------------|------------|
| customers | 1 | name | name, email, phone, rfc, address | — |
| suppliers | 1 | name | name, email, phone, rfc, address | — |
| products | 1 | name, price, cost, sku, category, stock | name, description | — |
| invoices | 1 | folio, customer_id, date, total, status | notes | customer_id → customers.id |
| orders | 1 | folio, customer_id, date, total, status | notes | customer_id → customers.id |
| payroll | 1 | name, department, salary, role, active | name, payment_method | — |

---

## Permisos y Entidades

Los permisos operan directamente sobre nombres de entidad. A diferencia del modelo anterior (namespace por entidad), ahora **todas las entidades comparten un topic gossip por org**:

1. **Resolución directa**: `can_open`/`can_write` contienen nombres de entidad (o `"*"`). Se resuelve con `can_access(lista, entidad)` → `lista.contains(entidad) || lista.contains("*")`.
2. **Validación por evento**: Cada evento recibido via gossip se valida contra `can_write` del rol del autor antes de indexarse.
3. **Validación en queries**: `query_entity()` verifica que el nombre de la entidad esté en `can_open` del rol del dispositivo.

Ver [TopicIds y Autorización](/arquitectura/namespaces/) para más detalles.
