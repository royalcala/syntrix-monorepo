# iroh-syntrix-docs — Rediseño v0.2.0

Estado: **implementado**.

## Qué cambió

| Aspecto | v0.1.0 | v0.2.0 |
|---------|--------|--------|
| Identidad | `author_id` (por persona) | `node_id` (por dispositivo) |
| Member | `{ author_id, active, role }` | `Device { node_id, active, role, person, name }` |
| Permisos | `read: [namespace]`, `write: [namespace]` | `can_open: [wildcards]`, `can_write: [namespaces]` |
| accept_cb | Un solo `org_id` fijo | Busca el org dueño del namespace vía `lookup_org()` |
| Registry | Responde: ¿qué namespaces leer/escribir? | Responde: ¿qué abrir? ¿puede escribir? ¿de qué org es este namespace? |
| Multi-org | `OrgId` como string | Registry con mapa `NamespaceId → OrgId` |
| Wildcards | No | `can_open: ["org_facturas_*"]` resuelto contra `known_namespaces` |
| Capabilities | No | `CapabilityManager`: encrypt/decrypt por device y role |

## Módulos

### `registry.rs` — NamespaceRegistry

```rust
impl NamespaceRegistry {
    // Población
    fn upsert_device(org_id, node_id, Device)
    fn remove_device(org_id, node_id)
    fn upsert_role(org_id, role, RoleGrants)
    fn set_known_namespaces(org_id, HashSet<String>)
    fn add_known_namespace(org_id, namespace)

    // Consultas para accept_cb
    fn is_device_active(org_id, node_id) -> bool
    fn map_namespace_to_org(NamespaceId, org_id)
    fn lookup_org(&NamespaceId) -> Option<OrgId>

    // Consultas para abrir namespaces
    fn openable_namespaces(org_id, node_id) -> HashSet<String>  // resuelve wildcards

    // Validación de escritura
    fn can_write(org_id, node_id, namespace) -> bool  // soporta wildcards y "*"

    // Multi-org
    fn org_ids() -> HashSet<OrgId>
}
```

`lookup_org()` usa un `HashMap<NamespaceId, OrgId>` interno. Se puebla cuando la app abre namespaces (al leer `org_<id>/control/namespaces/`). El `accept_cb` lo consulta para decidir.

### `accept.rs` — accept_cb multi-org

```rust
fn make_accept_cb(Arc<NamespaceRegistry>) -> impl Fn(NamespaceId, PublicKey) -> Ready<AcceptOutcome>
```

Flujo:
1. Recibe `(namespace, peer_public_key)`
2. `registry.lookup_org(&namespace)` → encuentra el `org_id`
3. `registry.is_device_active(&org_id, peer_bytes)` → Allow / Reject

### `capabilities.rs` — CapabilityManager

```rust
impl CapabilityManager {
    fn encrypt_for_device(capability, device_pubkey) -> EncryptedCapability
    fn encrypt_for_role(capability, role_key) -> EncryptedCapability
    fn decrypt_for_device(encrypted, device_secret) -> Option<Vec<u8>>
    fn decrypt_with_role_key(encrypted, role_key) -> Vec<u8>
    fn build_capability_map(capability, device_pubkeys, role_keys) -> HashMap<String, EncryptedCapability>
}
```

### `lib.rs`

```rust
pub type OrgId = String;
pub type NodeId = [u8; 32];
pub type Result<T> = anyhow::Result<T>;
```

## Tests

13 tests pasando:
- 5 de accept_cb (active, inactive, unknown, unmapped, multi-org)
- 4 de registry (active, openable with wildcards, can_write, can_write wildcard)
- 4 de capabilities (device encrypt/decrypt, wrong key, role encrypt/decrypt, build map)
