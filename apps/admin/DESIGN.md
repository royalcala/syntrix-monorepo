# syntrix-admin — Diseño

## Propósito

App de administración de la red Syntrix. Corre en **cualquier dispositivo** (desktop, laptop, teléfono, tablet) vía Tauri v2.

**Multi-org:** un mismo dispositivo puede administrar varias orgs (su propio negocio, la org de un cliente) y también ser miembro en orgs de otros usando syntrix-client. La app permite cambiar de org activa con un selector.

## Conceptos iroh

En iroh no hay usuarios ni IPs. La identidad es:

| Concepto | Qué es |
|----------|--------|
| **NodeId** | Hash de la llave pública Ed25519. Identifica un dispositivo en la red. |
| **Capability** | Clave criptográfica por namespace. Tenerla = poder abrir ese namespace. |
| **Entry firmado** | Cada entry en iroh-docs está firmada por el NodeId que la escribió. |

Un **dispositivo** = un NodeId. Una **persona** puede tener varios dispositivos (varios NodeIds). El admin los agrupa con el campo `person` en `org_control`.

```
Dispositivo A (celular de Alice)   NodeId: a1b2...
  ├── acme (admin)           ← Alice creó esta org, la administra
  └── clientex (sales)       ← Alice es miembro, factura

Dispositivo B (laptop de Alice)    NodeId: c3d4...
  ├── acme (admin)           ← mismo rol admin, distinto dispositivo
  └── clientex (sales)

Dispositivo C (PC de Bob)          NodeId: e5f6...
  ├── acme (contabilidad)    ← Bob trabaja para acme
  └── bob-org (admin)        ← Bob tiene su propio negocio
```

Cada dispositivo tiene UN solo NodeId pero puede participar en múltiples orgs con distintos roles en cada una.

## Cómo arranca todo

### Bootstrap: crear una org

```
1. Alice instala syntrix-admin en su celular → genera NodeId a1b2...
2. Toca "Crear organización"
3. Ingresa nombre (ej. "acme")
4. La app:
   a. Crea los namespaces org_acme/control, org_acme/data, org_acme/public, org_acme/private
   b. Escribe org_acme/control con:
      - members/a1b2...: { active: true, role: "admin", person: "alice" }
      - roles/admin: { can_open: ["*"], can_write: ["*"] }
      - org/: { name: "acme", created_at: ... }
   c. Guarda capabilities en disco (encriptados con PIN)
5. Alice puede crear más orgs o ser invitada a otras.
```

### Unirse a una org existente

```
1. Admin de "clientex" invita a Alice: registra su NodeId en org_clientex/control
2. Alice recibe notificación (o consulta periódicamente orgs donde aparece su NodeId)
3. Alice ve "clientex (sales)" en su lista de orgs
4. Cambia a esa org y usa syntrix-client o syntrix-admin según su rol
```

### Múltiples orgs, mismo dispositivo

```
syntrix-admin / syntrix-client:
  ┌─────────────────────────────┐
  │  Org selector: [acme   ▼]   │
  │  Org selector: [+ nueva]    │
  ├─────────────────────────────┤
  │  UI de la org activa        │
  └─────────────────────────────┘
```

Cambiar de org = cambiar de namespace prefix (`org_acme/` ↔ `org_clientex/`). Los capabilities son independientes por org. Los datos no se mezclan.

## Arquitectura

```
syntrix-admin/
├── src/                    ← Frontend React
│   ├── App.tsx             ← Org selector + router
│   ├── screens/
│   │   ├── Unlock.tsx      ← PIN para desbloquear capabilities locales
│   │   ├── Dashboard.tsx   ← Estado de la red, dispositivos conectados
│   │   ├── Devices.tsx     ← Lista de dispositivos, alta/baja
│   │   ├── DeviceEdit.tsx  ← Editar rol, person de un dispositivo
│   │   ├── Roles.tsx       ← Definir roles (can_open, can_write)
│   │   ├── Namespaces.tsx  ← Ver/ciclar namespaces
│   │   └── Settings.tsx    ← Relay, org name
│   └── components/
├── src-tauri/              ← Rust backend (Tauri)
│   ├── src/
│   │   ├── lib.rs          ← Tauri builder
│   │   ├── main.rs         ← Entry point
│   │   ├── identity.rs     ← Generar NodeId, guardar/cargar capabilities multi-org
│   │   ├── admin.rs        ← Leer/escribir org_<id>/control
│   │   └── bridge.rs       ← Always-on sync bridge
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

### Capas

```
┌──────────────────────────────────────────┐
│  ADMIN UI (React)                        │
├──────────────────────────────────────────┤
│  TAURI v2 (desktop + iOS + Android)      │
├──────────────────────────────────────────┤
│  IROH-SYNTRIX-DOCS                       │
│  NamespaceRegistry + accept_cb           │
├──────────────────────────────────────────┤
│  IROH-DOCS → IROH (P2P sync, gossip)     │
└──────────────────────────────────────────┘
```

No lleva LiveStore — solo lee y escribe `org_<id>/control` directamente en iroh-docs.

## org_control (multi-org)

Cada org tiene su propio namespace `org_<id>/control`. Write: admin de esa org, Read: todos los dispositivos activos en esa org.

```
iroh-docs namespaces:
  org_acme/control
  org_acme/data
  org_acme/public
  org_acme/private
  org_clientex/control
  org_clientex/data
  org_clientex/public
  org_boborg/control
  ...

Dentro de cada org_<id>/control:

  members/                    ← dispositivos en esta org
    <node_id_hex>/
      active: bool
      role: string
      name: string            ← etiqueta (ej. "Alice (laptop)")
      person: string          ← agrupa dispositivos
      joined_at: ISO8601

  roles/
    <role_name>/
      can_open: [namespace_id o wildcard, ...]
      can_write: [namespace_id, ...]

  namespaces/                 ← registro de ciclo de vida
    invoices_alice/
      status: "active" | "deprecated"
      created_at: ISO8601
      deprecated_at: ISO8601?
      replaced_by: "invoices_alice_v2"?
      writers: [node_id, ...]
      readers: [node_id, role_name, "*"]
      capabilities:
        <node_id_a>: <capability encriptado con pubkey de A>
        <node_id_b>: <capability encriptado con pubkey de B>
        <role_name>: <capability encriptado con clave del rol>

  org/
    name: string
    created_at: ISO8601
```

## Namespaces: sync-level isolation

Cada namespace tiene su capability key. Sin capability → no abrís → no recibís sync → dato no llega al disco.

### Template default (por org)

| Namespace | Write | Read (vía capability) | Contenido |
|-----------|-------|----------------------|-----------|
| `org_<id>/public` | Admin | Miembros | Productos, clientes |
| `org_<id>/data` | Miembros | Miembros | Invoices, órdenes |
| `org_<id>/private` | Admin | Admin + HR | Payroll |
| `org_<id>/control` | Admin | Miembros | Dispositivos, roles, namespaces |

Si se necesita privacidad por vendedor, se agregan namespaces `invoices_<person>` donde solo ese vendedor, admin y contabilidad leen.

## Control de lectura

El campo `readers` define quién recibe el capability. Acepta **node_ids** y **role names**.

```json
{
  "readers": ["abc111", "def222", "contabilidad"],
  "capabilities": {
    "abc111":        "<capability encriptado con pubkey de abc111>",
    "def222":        "<capability encriptado con pubkey de def222>",
    "contabilidad":  "<capability encriptado con clave del rol contabilidad>"
  }
}
```

### Wildcards en roles

```json
{
  "can_open": ["org_facturas_*", "org_public", "org_control"],
  "can_write": []
}
```

Cuando el admin crea `org_facturas_carlos`, contabilidad automáticamente recibe capability de lectura.

### Revocar lectura

Requiere rotar el capability del namespace. Datos viejos en disco del removido: se quedan (limitación aceptada).

## Validación de escritura (defensa en profundidad)

```
1. Dispositivo de Bob (sales) intenta write con type "payroll" en org_acme/data
2. Consulta org_acme/control → Bob rol sales → can_write: ["invoice", "order"]
3. "payroll" no está → RECHAZA
4. Si Bob fuerza el write, demás peers validan y descartan
```

## Flujos

### Agregar un dispositivo a una org

```
1. Admin en syntrix-admin → selecciona org → "Agregar dispositivo"
2. Admin pide el NodeId al empleado (QR, texto, presencial)
3. Admin ingresa: NodeId, nombre, person, role
4. Admin confirma → org_<id>/control se actualiza
5. Sync propaga → el dispositivo lee control → desencripta capabilities → abre namespaces
```

### Cambiar permisos (sin despedir)

```
1. Admin cambia role: "sales" → "intern"
2. Sync propaga
3. Dispositivo lee nuevo rol → can_write ahora es ["order"]
4. Intenta crear invoice → su propio cliente RECHAZA
5. Revertir: admin restaura role → sin rotar capabilities
```

### Dar de baja un dispositivo

```
1. Admin marca → active: false
2. Namespaces donde era writer: crea v2 con nuevo capability, v1 deprecated
3. Sync propaga → accept_cb bloquea handshake
4. Otros dispositivos de la misma persona siguen funcionando
```

### Perdió su dispositivo (admin de una org)

```
1. Admin consigue dispositivo nuevo → genera NUEVO NodeId
2. Ingresa frase de recuperación (capability de org_<id>/control en BIP39, guardado en papel)
3. Desencripta el capability → escribe org_<id>/control agregando su nuevo NodeId como admin
4. Sync normal desde cualquier peer
```

### Perdió su dispositivo (empleado)

```
1. Admin marca dispositivo perdido → active: false
2. Empleado instala app en dispositivo nuevo → genera NUEVO NodeId
3. Admin agrega el nuevo NodeId con mismo person y role
4. Dispositivo nuevo sync desde cualquier peer
```

## Always-on sync bridge

En mobile, iroh corre en background service. Mantiene conexión QUIC al relay, participa en gossip swarms de todas las orgs activas.

## Comandos Tauri

```rust
// Identidad
#[tauri::command]
fn generate_device_keys() -> String       // genera par Ed25519, retorna NodeId hex
#[tauri::command]
fn get_node_id() -> String

// Bootstrap
#[tauri::command]
fn create_org(name: String) -> Result<()>
#[tauri::command]
fn recover_admin(org_id: String, phrase: String) -> Result<()>

// Gestión de dispositivos (en la org activa)
#[tauri::command]
fn add_device(node_id: String, name: String, person: String, role: String) -> Result<()>
#[tauri::command]
fn update_device(node_id: String, active: bool, role: String) -> Result<()>

// Roles
#[tauri::command]
fn create_role(name: String, can_open: Vec<String>, can_write: Vec<String>) -> Result<()>
#[tauri::command]
fn update_role(name: String, can_open: Vec<String>, can_write: Vec<String>) -> Result<()>

// Lectura (org activa)
#[tauri::command]
fn list_devices() -> Vec<DeviceInfo>
#[tauri::command]
fn list_roles() -> Vec<RoleInfo>
#[tauri::command]
fn list_namespaces() -> Vec<NamespaceInfo>
#[tauri::command]
fn list_orgs() -> Vec<OrgInfo>

// Bridge
#[tauri::command]
fn network_status() -> NetworkStatus
```

## Seguridad

| Aspecto | Decisión |
|---------|----------|
| Identidad | NodeId = hash de Ed25519 pubkey. Una por dispositivo. |
| Capabilities en disco | Encriptados con clave derivada de PIN local |
| Comunicación | iroh QUIC (end-to-end encrypted). Relay no ve contenido. |
| org_control | Write = admin de esa org. Read = miembros activos de esa org. |
| Recuperación admin | Frase BIP39 resguarda el capability de org_control. Sin ella, no se recupera. |
| Recuperación empleado | Admin agrega el nuevo dispositivo. Sin frases. |

## Build targets

```bash
cargo tauri build                                  # Linux (.deb/.rpm/.AppImage)
cargo tauri build --target universal-apple-darwin  # macOS (.dmg)
cargo tauri build --target x86_64-pc-windows-msvc  # Windows (.exe/.msi)
cargo tauri android build                          # Android (.apk)
cargo tauri ios build                              # iOS (.ipa, macOS solamente)
```
