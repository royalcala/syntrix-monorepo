# iroh-syntrix-docs

Authorization wrapper for iroh-docs. Two things: decide **quién puede sincronizar** con nosotros y **qué namespaces puede abrir**.

## Cómo funciona en 30 segundos

```
Peer remoto quiere sincronizar conmigo
  → accept_cb: ¿está en org_control como miembro activo? → Allow / Reject
  → NamespaceRegistry: ¿qué namespaces puede leer/escribir según su rol?
  → Solo abro esos namespaces
```

## Los dos módulos

### 1. `NamespacerRegistry` (`registry.rs`)

Estructura en memoria que el admin llena leyendo el namespace `org_control`. Responde dos preguntas:

```rust
// ¿Este peer es miembro activo de la org?
registry.is_member_active("acme", &alice_key) → true / false

// ¿Qué namespaces puede leer alice?
registry.readable_namespaces("acme", &alice_key) → {"org_products", "org_customers"}

// ¿Qué namespaces puede escribir alice?
registry.writable_namespaces("acme", &alice_key) → {"invoices_alice"}
```

El admin puebla el registry desde el `org_control` namespace, que es un namespace normal de iroh-docs donde se escriben entradas así:

```
org_control/
├── members/<author_id>  →  { "active": true, "role": "sales" }
└── roles/sales          →  { "read": ["org_products"], "write": ["invoices_alice"] }
```

### 2. `accept_cb` (`accept.rs`)

Callback que se pasa a `iroh_docs::net::handle_connection`. Decide si aceptar o rechazar un handshake entrante:

```rust
let cb = make_accept_cb(registry, "acme".into());
// Se usa internamente en iroh-docs durante el handshake QUIC.
// Si el peer no está activo en la org → AcceptOutcome::Reject
```

## Flujo completo

```
1. Admin crea org_control con miembros y roles
2. Admin escribe en iroh-docs: set_bytes("org_control/members/alice", payload)
3. Iroh-docs sincroniza org_control con todos los peers
4. Cada peer lee org_control y llena su NamespaceRegistry
5. Cuando un peer se conecta:
   a. accept_cb verifica membresía activa → Allow/Reject
   b. NamespaceRegistry decide qué namespaces abrir
   c. Solo se syncan los namespaces autorizados
```

## Ejemplo de uso

```rust
use iroh_syntrix_docs::registry::{NamespaceRegistry, Member, RoleGrants};
use iroh_syntrix_docs::accept::make_accept_cb;

let mut registry = NamespaceRegistry::new();

// Admin agrega a alice
registry.upsert_member("acme".into(), alice_id, Member {
    author_id: alice_id,
    active: true,
    role: "sales".into(),
});

// Admin define el rol sales
registry.upsert_role("acme".into(), "sales".into(), RoleGrants {
    read: vec!["org_products".into(), "org_customers".into()],
    write: vec!["invoices_alice".into()],
});

// Crear el accept callback para pasar a iroh-docs
let accept_cb = make_accept_cb(
    std::sync::Arc::new(registry),
    "acme".into(),
);
```

## Limitaciones

- El registry es **en memoria** (no persiste). En producción, cada vez que arranca la app, el admin debe re-leer `org_control` y re-poblar el registry.
- No maneja cambios en caliente del `org_control`. Si el admin cambia un rol mientras la app corre, hay que re-sincronizar el registry manualmente.
- Quien tuvo datos localmente los conserva. Si un empleado es despedido, deja de recibir sync nuevo, pero los datos que ya bajó quedan en su disco.
- No hay revocación criptográfica de escritura (por eso los namespaces son de un solo escritor).
