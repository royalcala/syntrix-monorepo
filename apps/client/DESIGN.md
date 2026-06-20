# syntrix-client — Diseño

## Propósito

App cliente del ERP Syntrix. Corre en **cualquier dispositivo** (desktop, laptop, teléfono, tablet) vía Tauri v2.

Cada empleado (incluido el admin) usa syntrix-client para facturar, consultar catálogos, ver reportes. El admin administra desde syntrix-admin, pero también factura desde syntrix-client en su laptop.

## Conceptos iroh

| Concepto | Qué es |
|----------|--------|
| **NodeId** | Hash de la llave pública Ed25519. Identifica un dispositivo. |
| **Capability** | Clave por namespace. Tenerla = abrir el namespace. Sin ella, el dato ni se sincroniza. |
| **Entry firmado** | Cada entry en iroh-docs está firmada por el NodeId que la escribió. |

Un dispositivo = un NodeId. Una persona puede tener varios. El dispositivo genera sus llaves localmente al instalarse. Sin frases semilla.

## Multi-org

Un mismo dispositivo participa en múltiples orgs con distintos roles:

```
Dispositivo A (celular de Alice)   NodeId: a1b2...
  ├── acme (admin)           ← Alice administra acme
  └── clientex (sales)       ← Alice factura para clientex

Dispositivo B (laptop de Alice)    NodeId: c3d4...
  ├── acme (admin)           ← mismo rol admin, distinto dispositivo
  └── clientex (sales)
```

Namespaces prefijados por org: `org_acme/control`, `org_acme/data`, `org_clientex/data`, etc.

## Arquitectura

```
syntrix-client/
├── src/                    ← Frontend React (UI del ERP)
│   ├── App.tsx             ← Org selector + router
│   ├── screens/
│   │   ├── Setup.tsx       ← Primer arranque: genera llaves, muestra NodeId
│   │   ├── Dashboard.tsx   ← Resumen, notificaciones, estado de sync
│   │   ├── Invoices.tsx    ← Lista de invoices
│   │   ├── InvoiceForm.tsx ← Crear/editar invoice
│   │   ├── Products.tsx    ← Catálogo (readonly)
│   │   ├── Customers.tsx   ← Clientes (readonly)
│   │   └── Settings.tsx    ← Perfil, orgs, estado de sync
│   └── components/
├── src-tauri/              ← Rust backend (Tauri)
│   ├── src/
│   │   ├── lib.rs          ← Tauri builder, comandos
│   │   ├── main.rs         ← Entry point
│   │   ├── identity.rs     ← Generar NodeId, desencriptar capabilities
│   │   ├── events.rs       ← HLC + commit de eventos
│   │   └── sync.rs         ← SyncBackend Iroh adapter, estado de sync
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

### Capas

```
┌──────────────────────────────────────────┐
│  ERP UI (React)                          │
├──────────────────────────────────────────┤
│  LIVESTORE (TypeScript, SQLite wasm)     │
│  Event sourcing: commit → materialize    │
│  SyncBackend → Iroh Doc (local)          │
├──────────────────────────────────────────┤
│  TAURI v2 (desktop + iOS + Android)      │
├──────────────────────────────────────────┤
│  IROH-SYNTRIX-DOCS                       │
│  NamespaceRegistry + accept_cb           │
├──────────────────────────────────────────┤
│  IROH-DOCS → IROH (P2P sync)            │
└──────────────────────────────────────────┘
```

### Disco local

```
~/.syntrix/
├── iroh-data/          ← iroh (redb, blobs, gossip)
├── livestore/          ← SQLite (event log + state)
└── config/             ← capabilities encriptados con PIN
```

## First run (dispositivo nuevo)

```
1. Usuario instala syntrix-client
2. La app genera par de llaves Ed25519 LOCALMENTE
3. Muestra: "Tu device ID: 7a3b... — compartilo con tu admin"
4. Usuario comparte el NodeId con el admin (QR, texto, presencial)
5. Admin usa syntrix-admin para registrar el dispositivo en org_<id>/control
6. Sync propaga org_control
7. El dispositivo lee org_<id>/control, encuentra capabilities encriptados para su NodeId
8. Desencripta capabilities con su llave privada
9. Abre namespaces (org_<id>/data, org_<id>/public, org_<id>/control)
10. iroh-docs sync arranca → Livestore materializa SQLite
11. UI muestra dashboard con org selector

Sin frase semilla. Sin identity vault.
El dispositivo ES su propia identidad.
```

## Namespaces

Cada org tiene sus propios namespaces. El dispositivo abre los que `org_<id>/control` le autoriza.

```
org_<id>/data       Write: miembros    Read: miembros      ← invoices, órdenes
org_<id>/public     Write: admin       Read: miembros      ← productos, clientes
org_<id>/private    Write: admin       Read: admin+HR      ← NO se abre sin capability
org_<id>/control    Write: admin       Read: miembros      ← permisos, capabilities
```

### Apertura de namespaces al arrancar

```
1. Lee org_<id>/control → itera namespaces/ con status: "active"
2. Para cada uno, busca capabilities encriptados para su NodeId o su role
3. Desencripta con su llave privada (por NodeId) o clave del rol
4. Abre los namespaces con los capabilities
5. iroh-docs sync solo para esos namespaces
```

## Eventos

Cada evento va al namespace que corresponda, con key HLC + NodeId:

```
Key:  "org_acme/data/evt:<hlc_ts>:<hlc_count>:<node_id_hex>"
Value: JSON { type: "invoice", payload: {...} }
```

### HLC (Hybrid Logical Clock)

```rust
struct Hlc {
    ts: u64,        // microsegundos desde epoch
    count: u32,     // contador mismo microsegundo
    node: [u8; 32], // NodeId del escritor
}
```

### Schemas

```
invoice:   { customer_id, items: [{product_id, qty, price}], total, currency, date }
product:   { name, sku, price, unit, category }
customer:  { name, tax_id, address, phone, email }
order:     { customer_id, items: [{product_id, qty}], status, date }
```

### Validación de escritura (doble capa)

```
Capa 1 — capabilities: sin capability del namespace → ni se emite
Capa 2 — validación local: el peer consulta org_<id>/control antes de escribir
  1. Dispositivo de Bob (sales) intenta write con type "payroll" en org_acme/data
  2. Consulta org_acme/control → Bob rol sales → can_write: ["invoice", "order"]
  3. "payroll" no está → RECHAZA
  4. Si Bob fuerza el write, demás peers validan y descartan
```

## Flujo de sync

```
Peer online:
  1. Escribe evento en Iroh Doc local (org_<id>/data)
  2. iroh-docs set reconciliation con peers del swarm
  3. Livestore SyncBackend detecta nuevo evento → materializa SQLite
  4. UI reacciona vía queries reactivas

Peer offline:
  1. Escribe localmente (Iroh Doc + SQLite)
  2. Cuando vuelve online → se conecta al relay
  3. iroh-docs sync de diffs con cualquier peer online
  4. Gossip notifica a los demás
```

## Comandos Tauri

```rust
// Identidad (por dispositivo)
#[tauri::command]
fn generate_device_keys() -> String         // genera par Ed25519, retorna NodeId hex
#[tauri::command]
fn get_node_id() -> String                  // NodeId de este dispositivo

// Multi-org
#[tauri::command]
fn list_orgs() -> Vec<OrgInfo>              // orgs donde este dispositivo es miembro
#[tauri::command]
fn set_active_org(org_id: String)           // cambiar de org activa

// Eventos
#[tauri::command]
fn commit_event(event_type: String, payload: String) -> Result<String>

// Sync
#[tauri::command]
fn sync_status() -> SyncStatus              // peers conectados, pendientes, last sync

// Lectura (Livestore queries)
#[tauri::command]
fn query_invoices(filter: String) -> Vec<Invoice>
#[tauri::command]
fn query_products() -> Vec<Product>
#[tauri::command]
fn query_customers() -> Vec<Customer>
```

## Build targets

```bash
cargo tauri dev                                   # desarrollo con HMR
cargo tauri build                                  # Linux
cargo tauri build --target universal-apple-darwin  # macOS
cargo tauri build --target x86_64-pc-windows-msvc  # Windows
cargo tauri android build                          # Android
cargo tauri ios build                              # iOS (macOS)
```
