# Decisión final de arquitectura de sync — DeepSeek

## Consenso entre los 4 modelos

Los 4 modelos (ChatGPT, Gemini, Claude, DeepSeek) coinciden en lo estructural. Las diferencias son de alcance MVP.

| Punto | ChatGPT | Gemini | Claude | DeepSeek |
|-------|---------|--------|--------|----------|
| Arquitectura base | Opción C | Opción C | Opción C | Opción C |
| Query layer | TanStack DB adapter | TanStack DB adapter | TanStack DB adapter | TanStack DB adapter |
| iroh-streams | 🥈 muy ambicioso | — | ❌ descartar | Fase 2 |
| Overrides | ❌ eliminar | ❌ pausar | — | ❌ eliminar |
| private_* en MVP | solo si necesario | — | ❌ eliminar | ❌ eliminar |
| StreamDB | — | — | ❌ descartar ahora | ❌ descartar ahora |
| Namespaces MVP | 3 | 6 (sin overrides) | 5 (sin private) | **4** |

---

## La decisión

### Stack final

```
iroh-docs (P2P sync, storage redb)
  ↓
4 namespaces: control, catalogs, operational, payroll
  ↓
adapter ~200 LOC (namespaces → TanStack DB collections, filtro por key prefix)
  ↓
TanStack DB (queries reactivas, differential dataflow, useLiveQuery)
  ↓
React + TanStack Router + shadcn
```

### Namespaces

```
control       ← miembros, roles, permisos. Read: todos los activos. Write: admin.
catalogs      ← productos, clientes, plan de cuentas. Read: todos. Write: admin.
operational   ← facturas, órdenes, notas de venta. Read: admin + contab + sales. Write: según rol.
payroll       ← nómina. Read: admin + HR + contab. Write: admin.
```

Todo dentro de `operational` con key prefixes por tipo y usuario:

```
operational/
  invoices/alice/evt:...
  invoices/bob/evt:...
  orders/alice/evt:...
  orders/bob/evt:...
  sales_notes/alice/evt:...
```

El adapter lee entries de `operational`, filtra por prefix (`invoices/`) y las inserta en la collection correspondiente de TanStack DB. Una collection por tipo de dato.

### Lo que eliminamos del MVP

**1. Namespace `private_*`** (coinciden DeepSeek, Claude y ChatGPT)

Las preferencias de usuario (tema, layout, settings) no necesitan un doc iroh con ticket propio. Van en `localStorage`/`IndexedDB` del dispositivo. Si el usuario quiere sincronizar preferencias entre dispositivos, se agrega `private_*` en Fase 2.

**2. Permisos por usuario (overrides)** (coinciden DeepSeek, ChatGPT y Gemini)

Sin overrides en `members/<node_id>`. Solo permisos por rol. Si Bob necesita acceso excepcional a payroll, admin le cambia el rol temporalmente. Más simple, más auditable, menos bugs. El motor de reglas IAM (¿qué pesa más, el rol o el override?) no se justifica en MVP.

**3. iroh-streams** (coinciden DeepSeek, Claude y ChatGPT)

Construir un protocolo de streams + servidor HTTP + sync engine P2P desde cero para evitar 200 LOC de adapter es inversión negativa. Se documenta como opción Fase 2, se descarta para MVP.

**4. StreamDB / Durable Streams** (coinciden DeepSeek y Claude)

Excelente tecnología, pero requiere stream server (rompe el modelo P2P sin servidor). Si Syntrix pivota a server-first con clientes web, se reevalúa. Guardado como referencia.

### Lo que mantenemos

| Componente | ¿Se queda? | Nota |
|-----------|-----------|------|
| `iroh-docs` | ✅ | P2P sync + redb storage. Ya implementado. |
| `org_control` con `members/<node_id>` + `roles/<role>` | ✅ | Ya implementado en `admin.rs`. |
| `NamespaceRegistry` con `can_write` / `can_open` | ✅ | Ya implementado en `registry.rs`. Con wildcards. |
| `accept_cb` | ✅ | Ya implementado en `accept.rs`. Bloquea peers inactivos. |
| `syntrix-docs` (capabilities encriptadas) | ✅ | Ya implementado en `capabilities.rs`. |
| `create_org()` → 4 namespaces | ✅ | Cambio menor: crear 4 docs en vez de 2. |
| `add_device()` → tickets selectivos | ✅ | Cambio menor: compartir tickets por namespace según rol. |
| `commit_event()` con `can_write()` | ✅ | Agregar validación de 5 líneas. |
| TanStack DB adapter | ✅ | ~200 LOC. Mapea namespaces → collections. |
| TanStack DB queries | ✅ | `useLiveQuery`, joins, optimistic mutations. |
| Zod schemas | ✅ | Validación al insertar en collections. |
| Migraciones en deserialización | ✅ | `deserializeEntry()` transforma entries viejas al vuelo. |

---

## Lo que queda por implementar

### Cambios en el código existente (Rust)

**1. `create_org()` — de 2 docs a 4** (`admin.rs:6-32`)

```rust
// Antes: control_doc + data_doc
// Ahora: control, catalogs, operational, payroll
let control_doc    = api.create().await?;
let catalogs_doc   = api.create().await?;
let operational_doc = api.create().await?;
let payroll_doc    = api.create().await?;

// Registrar en registry para accept_cb
state.map_namespace(control_doc.id(), org_id);
state.map_namespace(catalogs_doc.id(), org_id);
state.map_namespace(operational_doc.id(), org_id);
state.map_namespace(payroll_doc.id(), org_id);
```

**2. `add_device()` — tickets selectivos** (`admin.rs:34-65`)

Según el rol del dispositivo, compartir solo los tickets de los namespaces que `can_open` permite. Sales recibe `control` + `catalogs` + `operational`. No recibe `payroll`.

**3. `commit_event()` — validación de escritura** (`events.rs:40-64`)

Antes de escribir, verificar `registry.can_write(org_id, node_id, namespace)`. Si el rol no tiene `can_write` para ese namespace, rechazar.

**4. Role grants simplificados** (`identity.rs:143-149`)

```rust
fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants {
            can_open: vec!["control", "catalogs", "operational", "payroll"],
            can_write: vec!["catalogs", "operational", "payroll"],
        },
        "sales" => RoleGrants {
            can_open: vec!["control", "catalogs", "operational"],
            can_write: vec!["operational"],
        },
        "contabilidad" => RoleGrants {
            can_open: vec!["control", "catalogs", "operational", "payroll"],
            can_write: vec![],
        },
        "hr" => RoleGrants {
            can_open: vec!["control", "catalogs", "payroll"],
            can_write: vec!["payroll"],
        },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}
```

### Código nuevo (TypeScript)

**5. TanStack DB adapter (~200 LOC)**

```typescript
// irohCollectionOptions.ts
// Un adapter que:
//  - sync(): lee entries de operational doc, filtra por key prefix, inserta en collection
//  - onInsert/onUpdate/onDelete: llama invoke("commit_event") con el namespace correcto
//  - subscribe(): escucha eventos de iroh-docs para tiempo real
//  - deserializeEntry(): migraciones al vuelo + Zod parse
```

**6. Schema definitions (Zod)**

```typescript
// schemas.ts
const invoiceSchema = z.object({ ... })
const productSchema = z.object({ ... })
const customerSchema = z.object({ ... })
// etc.
```

**7. Collections setup**

```typescript
// collections.ts
export const invoicesCollection = createCollection(irohCollectionOptions({
  dataType: "invoices",
  schema: invoiceSchema,
  getKey: (inv) => inv.id,
}))

export const productsCollection = createCollection(irohCollectionOptions({
  dataType: "products",
  schema: productSchema,
  getKey: (p) => p.id,
}))
// ... etc
```

---

## Tiempo estimado

| Tarea | Esfuerzo |
|-------|----------|
| `create_org()` → 4 docs | 30 min |
| `add_device()` tickets selectivos | 1 h |
| `commit_event()` validación | 15 min |
| Role grants actualizados | 15 min |
| TanStack DB adapter | 1 día |
| Zod schemas | 2 h |
| Collections setup | 1 h |
| Tests (E2E: privacidad payroll) | 2 h |
| **Total** | **~2 días** |

---

## Fase 2 (cuando se necesite)

| Feature | Gatillo |
|---------|---------|
| `private_*` namespaces | Usuarios piden sync de preferencias entre dispositivos |
| Permisos por usuario (overrides) | Cliente enterprise exige permisos granulares |
| `iroh-streams` | Se requiere web client sin Tauri, o se acepta servidor |
| `StreamDB` directo | Se pivota a modelo server-first |
| `audit`, `reports` namespaces | Módulos nuevos requieren privacidad adicional |
| Números de factura sin colisiones offline | Gemini lo preguntó — requiere diseño aparte |

---

## Lo que NO hacemos

- **Namespace por escritor** — escala mal, gestionar 100+ namespaces no es viable
- **1 solo namespace** — payroll viaja a todos los dispositivos, inaceptable
- **iroh-streams desde cero** — ratio esfuerzo/beneficio negativo para MVP
- **StreamDB como reemplazo de iroh-docs** — requiere servidor, rompe P2P
- **Overrides por dispositivo** — complejidad innecesaria en MVP
- **Motor de reglas IAM** — roles simples cubren el 100% de los casos MVP

---

## Principio rector

> Menos infraestructura, más libertad operativa. El valor para el nodo está en facturas, inventario y reportes — no en si el log se sincroniza por un protocolo perfecto.

— Consenso de los 4 modelos.
