---
title: "Registry de Esquemas"
description: "Definición centralizada de entidades, campos, índices y relaciones — source of truth para toda la aplicación."
---

# Registry de Esquemas

Syntrix utiliza un **registry de esquemas centralizado** definido en el crate `syntrix-schema` (Rust). Este registry es la **única fuente de verdad** para:

- Generación de índices secundarios en redb (qué campos indexar).
- Selección de campos buscables en Tantivy (full-text).
- Migraciones de esquema (upcasters).
- Exportación JSON para el frontend (SchemaExplorer, codegen de tipos Zod).
- Visualización de relaciones entre entidades en el Admin Console.

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

## Definición de una Entidad

Cada entidad se define con sus campos, tipos, atributos de índice y relaciones:

```rust
pub fn invoices_schema() -> EntitySchema {
    entity!("invoices", 1, [
        field!(string: "id"),
        field!(sort_key_indexed: "folio"),
        field!(relation: "customer_id", target: "customers", field: "id"),
        field!(date_indexed: "date"),
        field!(date: "due_date"),
        field!(number_indexed: "total"),
        field!(number: "subtotal"),
        field!(number: "tax"),
        field!(indexed: "status"),
        field!(bool: "paid"),
        field!(text: "notes"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_status", ["status"]),
        index!("by_status_date", ["status", "date"]),
    ])
}
```

### Atributos de Campo
| Atributo | Descripción | Efecto |
|----------|-------------|--------|
| `string:` | Campo de texto sin índice | Solo almacenamiento |
| `text:` | Campo de texto buscable en Tantivy | Indexado en redb + Tantivy |
| `searchable:` | Campo indexado y buscable | Índice redb + Tantivy |
| `indexed:` | Índice secundario en redb | Búsquedas exactas por prefijo |
| `sort_key_indexed:` | Índice + orden por defecto | Usado como `ORDER BY` |
| `number:` | Campo numérico sin índice | Solo almacenamiento |
| `number_indexed:` | Campo numérico indexado | Codificación sortable big-endian |
| `date:` / `date_indexed:` | Campo de fecha | Fecha ISO 8601 |
| `bool:` | Campo booleano | `true`/`false` |
| `relation:` | Relación foránea | Crea índice + metadata de relación |

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

## Migraciones (Upcasters)

Cada evento lleva `schema_version`. Cuando el indexador encuentra un evento con versión anterior a la actual, aplica la cadena de upcasters deterministas definidos en `upcast.rs`:

```rust
pub struct CustomerV1ToV2;
impl Upcaster for CustomerV1ToV2 {
    fn from_version(&self) -> u32 { 1 }
    fn to_version(&self) -> u32 { 2 }
    fn upcast(&self, payload: serde_json::Value) -> serde_json::Value {
        // Transforma address string en objeto estructurado
    }
}
```

Los upcasters se registran en `collect_upcasters()` y se ejecutan en orden secuencial:

```
v1 → upcaster_v1_to_v2 → v2 → upcaster_v2_to_v3 → ... → vCurrent
```

---

## Exportación al Frontend

El comando Tauri `get_schema_registry()` devuelve el registry completo como JSON. El Admin Console lo consume en la pantalla **Explorador de Esquemas** para visualizar entidades, campos, índices y relaciones en un formato interactivo.

Para codegen de tipos TypeScript/Zod, se puede usar la exportación JSON del registry como entrada para un generador (pendiente de implementar en `pnpm gen:schema`).

---

## Consultas Schema-Driven

El motor de consultas (`query_entity_advanced`) usa el registry para:
1. Validar que los campos solicitados existen en el esquema.
2. Elegir entre índices simples (`INDEXES`) o compuestos (`COMPOSITE`).
3. Codificar valores de filtro con `encode_value()` para búsquedas correctas (especialmente numéricas).
4. Aplicar paginación server-side.

Las entidades legacy sin schema registrado caen en modo de compatibilidad inversa (indexación de todos los campos).

---

## Permisos y Entidades

Cada entidad tiene su propio namespace de Iroh. Los permisos operan directamente sobre nombres de entidad:

1. **Resolución directa**: `can_open`/`can_write` contienen nombres de entidad (o `"*"`). Se resuelve con `can_access(lista, entidad)` → `lista.contains(entidad) || lista.contains("*")`.
2. **Ticket sharing**: `send_invite()` itera `can_open` y genera tickets solo para los namespaces (entidades) que el rol necesita.
3. **Validación en queries**: `query_entity()` verifica que el nombre de la entidad esté en los namespaces abiertos del rol del dispositivo.

Ejemplo: Un rol `sales` con `can_open: ["customers","products","invoices","orders"]` puede abrir exactamente esos 4 namespaces. `query_entity("payroll")` es rechazado porque `payroll` no está en `can_open`.

Ver [Namespaces y Autorización](/arquitectura/namespaces/) para más detalles sobre el modelo de permisos.
