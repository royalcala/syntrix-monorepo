# Plan: Replace iroh-docs with Gossip-based Event Bus

## Goal

Eliminate `iroh-docs` (fork `royalcala/iroh-docs`) and `iroh-blobs` dependencies. Replace doc-based P2P sync with `iroh-gossip` pub/sub + `redb` as sole storage engine. Fix sync reliability, eliminate private fork, reduce storage layers from 3 to 1.

## Architecture Overview

```
BEFORE                                    AFTER
┌─────────────┐                          ┌─────────────┐
│  iroh-docs  │ (CRDT docs)              │  iroh-gossip │ (pub/sub)
│  iroh-blobs │ (blob storage)           │              │
│  redb       │ (local index)            │  redb        │ (sole storage)
└─────────────┘                          └─────────────┘
  3 storage engines                         1 storage engine

Sync: doc.start_sync + subscribe        Sync: gossip.broadcast + stream Receive
Storage: set_bytes(key, val)            Storage: redb EVENT_LOG + DOCUMENTS + INDEXES
```

### New data flow

```
commit_event (local)
  → validate can_write (existing logic)
  → write EVENT_LOG row in redb
  → upsert materialized document in redb DOCUMENTS
  → update INDEXES / COMPOSITE / HLC_TRACKER in redb
  → gossip.broadcast(serialized_event) to org topic
  → store in-in-memory events buffer (for local sync_pull)

gossip Received event (remote)
  → validate sender is known active member (redb members:{org}:{node})
  → validate sender's role.can_write includes entity (redb roles:{org}:{role})
  → validate HLC > existing HLC for same doc_id (redb HLC_TRACKER)
  → reject if any check fails
  → write EVENT_LOG + upsert document + indexes in redb
```

## Key Design Decisions

| # | Decision | Choice |
|---|---|---|
| 1 | Remove iroh-docs | Yes - fork is fragile, sync is unreliable |
| 2 | Remove iroh-blobs | Yes - no longer needed without docs |
| 3 | Keep iroh (networking) | Yes - P2P transport, identity, relays |
| 4 | Keep iroh-gossip | Yes - replaces start_sync + subscribe |
| 5 | Keep Tantivy | Yes - full-text search unchanged |
| 6 | Storage engine | redb (already integrated, pure Rust, zero new deps) |
| 7 | Catch-up sync | Point-to-point via endpoint.connect() to admin |
| 8 | Write permission | Local validation in commit_event (already exists) |
| 9 | Read permission (remote) | App-level: validate sender's can_write on receive |
| 10 | Multi-org / multi-admin | Isolation by TopicId (UUID per org) |
| 11 | Direct vs relay | iroh N0: direct priority, relay fallback (inherited) |
| 12 | Author identity | PublicKey from SecretKey (no separate AuthorId) |
| 13 | TopicId per org | UUID [u8; 32] generated at org creation |
| 14 | Heartbeat | gossip broadcast + redb storage |
| 15 | Invite payload | { org_name, role, topic_id, admin_addr } (no tickets) |
| 16 | ALPN for catch-up | /syntrix/catchup/1 |

## Task List

### Phase 1: syntrix-core crate cleanup

- [ ] **T1.** `crates/syntrix-core/Cargo.toml`: Remove `iroh-docs`, `iroh-blobs`; add `redb = "2"`.
- [ ] **T2.** Delete `crates/syntrix-core/src/accept.rs` — no docs to accept.
- [ ] **T3.** Delete `crates/syntrix-core/src/capabilities.rs` — unused everywhere.
- [ ] **T4.** `crates/syntrix-core/src/registry.rs`: Remove `NamespaceId` imports and `namespace_org_map: HashMap<NamespaceId, OrgId>`. Add `topic_ids: HashMap<OrgId, TopicId>`. Update `map_namespace_to_org` → `map_topic_to_org`. Add `get_topic_id(org_id) -> Option<TopicId>`.
- [ ] **T5.** `crates/syntrix-core/src/sync.rs`: Rewrite `get_sync_info` — read members and heartbeats from redb tables instead of doc queries. Accept `RelationalEngine` instead of `Doc` + `Store`.
- [ ] **T6.** `crates/syntrix-core/src/heartbeat.rs`: Rewrite — broadcast `control.heartbeat` event via gossip sender instead of `doc.set_bytes`. Accept `GossipSender` instead of `Doc`.
- [ ] **T7.** `crates/syntrix-core/src/lib.rs`: Remove `pub mod accept;` and `pub mod capabilities;` and their re-exports.

### Phase 2: Index engine extensions (apps/client)

- [ ] **T8.** `apps/client/src-tauri/src/indexes.rs`: Add new table definition:
  ```
  const EVENT_LOG: TableDefinition<&str, &[u8]> = TableDefinition::new("event_log");
  // Key = "evt:{org_id}:{hlc_ts:020}:{hlc_count:08}:{hlc_node}"
  // Value = JSON: { type, hlc, schema_version, payload }
  ```
- [ ] **T9.** `apps/client/src-tauri/src/indexes.rs`: Add method `append_event(&self, org_id, event_json) -> Result<()>`.
- [ ] **T10.** `apps/client/src-tauri/src/indexes.rs`: Add method `query_events_since(org_id, cursor_ts) -> Result<Vec<EventEntry>>` for audit and catch-up.
- [ ] **T11.** `apps/client/src-tauri/src/indexes.rs`: Add tables for control data:
  ```
  MEMBERS:  key = "members:{org_id}:{node_id_hex}" → JSON { active, role, person, name, device_addr }
  ROLES:    key = "roles:{org_id}:{role_name}" → JSON { can_open, can_write }
  HEARTBEATS: key = "heartbeat:{org_id}:{node_id_hex}" → JSON { ts, status }
  ```

### Phase 3: New modules

- [ ] **T12.** Create `apps/client/src-tauri/src/gossip.rs` — `GossipEventBus` struct:
  - `topics: HashMap<OrgId, (GossipSender, JoinHandle)>`
  - `join_org(org_id, topic_id, bootstrap_peers) -> Result<()>`
  - `broadcast(org_id, event_bytes) -> Result<()>`
  - Spawns receive loop per topic that validates + indexes remote events
- [ ] **T13.** Create `apps/client/src-tauri/src/catchup.rs` — `CatchupProtocol`:
  - Implements `ProtocolHandler` for ALPN `/syntrix/catchup/1`
  - Server side: reads event log from redb since HLC cursor, streams to peer
  - Client side: `request_catchup(admin_addr, org_id, since_hlc) -> Result<()>`
  - Validates + indexes each received event

### Phase 4: Client app rewrite

- [ ] **T14.** `apps/client/src-tauri/src/identity.rs`: Major rewrite of `AppState`:
  - Remove: `docs_api: DocsApi`, `_store`, `author: AuthorId`, `events: HashMap<...>`
  - Add: `gossip: GossipEventBus`, `catchup: CatchupProtocol`
  - `AppState::new_with_data_dir()`: Remove `Docs::persistent()`, `api.author_create()`, doc opening, member/role loading from control doc. Keep keypair generation, endpoint, gossip init, redb init. Load orgs from config (topic_id instead of control_id + namespace_ids).
  - `add_org_docs()` → `add_org(org_id, name, role, topic_id, peers)`
  - `get_org_docs()` → `get_org(org_id)` returns redb-backed org state
  - Remove `record_event()`, `get_events_since()`, `sync_and_populate_org_members()`
- [ ] **T15.** `apps/client/src-tauri/src/events.rs`: Rewrite `commit_event`:
  - Remove: doc lookup, `doc.set_bytes()`, `block_on`
  - Keep: role validation, HLC generation, payload serialization
  - Add: `indexer.append_event()` + `indexer.upsert_document_with_hlc()` + `gossip.broadcast()`
  - Return HLC key on success
- [ ] **T16.** `apps/client/src-tauri/src/sync.rs`: Rewrite `sync_pull`:
  - Remove: `AppState.events` HashMap reads
  - Use: `indexer.query_events_since(org_id, cursor_ts)`
  - Remove `SyncEventEncoded` (iroh-docs specific), keep `SyncPullResult`
- [ ] **T17.** `apps/client/src-tauri/src/invite.rs`: Update `InvitePayload`:
  - Remove `tickets: Vec<TicketInfo>`
  - Add `topic_id: [u8; 32]`
- [ ] **T18.** `apps/client/src-tauri/src/audit.rs`: Rewrite — `scan_doc_events` → `indexer.query_events_since()` with filter in Rust.
- [ ] **T19.** `apps/client/src-tauri/src/lib.rs`: Update `join_org` command:
  - Remove: ticket parsing, `api.import()`, `doc.start_sync()`
  - Add: `gossip.join_org(topic_id, bootstrap)` + `catchup.request_catchup(admin_addr, org_id, 0)`
  - Remove `start_doc_subscriptions()` call — gossip receive loop replaces it
- [ ] **T20.** `apps/client/src-tauri/src/lib.rs`: Update Tauri command handlers:
  - `sync_pull` → reads from redb (same signature)
  - `commit_event` → writes redb + broadcasts gossip (same signature)
  - `get_sync_info` → reads redb members/heartbeats (same signature)
  - Register `CatchupProtocol` on Router with ALPN `/syntrix/catchup/1`
- [ ] **T21.** `apps/client/src-tauri/Cargo.toml`: Remove `iroh-docs`, `iroh-blobs`. Keep `iroh`, `iroh-gossip`, `redb`.

### Phase 5: Admin app rewrite

- [ ] **T22.** `apps/admin/src-tauri/src/identity.rs`: Rewrite `AppState`:
  - Remove: `docs_api`, `_store`, `author`, doc-based org loading
  - Add: `gossip: GossipEventBus`
  - `AppState::new_with_data_dir()`: No `Docs::persistent()`, no doc opening. Keep keypair, endpoint, gossip.
  - `add_org(name, topic_id, ...)` — stores org in local map
  - Remove `sync_and_populate_org_members()` — gossip handles sync
- [ ] **T23.** `apps/admin/src-tauri/src/admin.rs`: Rewrite all org/doc operations:
  - `create_org()`: Generate TopicId UUID. Write admin member/role to redb. Broadcast via gossip.
  - `add_device()`: Write member entry to redb. Broadcast `control.member.updated` via gossip.
  - `update_device()`: Same pattern.
  - `share_org_tickets()`: Remove entirely (no tickets). Replace with `get_invite_info()` that returns topic_id + admin_addr.
  - `send_invite()`: Update payload to use topic_id instead of tickets.
- [ ] **T24.** `apps/admin/src-tauri/src/audit.rs`: Rewrite — same as client: `indexer.query_events_since()`.
- [ ] **T25.** `apps/admin/src-tauri/Cargo.toml`: Remove `iroh-docs`, `iroh-blobs`.

### Phase 6: Index engine for admin (shared with client via syntrix-core)

- [ ] **T26.** Move `indexes.rs` (RelationalEngine, including EVENT_LOG + control tables) to `syntrix-core` so both apps share one copy. Or duplicate — simpler to keep in client and copy to admin. Choose duplication for simplicity (no breaking change to syntrix-core structure).

### Phase 7: Testkit rewrite + E2E tests

- [ ] **T27.** `crates/syntrix-testkit/Cargo.toml`: Remove `iroh-docs`, `iroh-blobs`. Add `redb = "2"`, `iroh-gossip = "=0.100.0"`.
- [ ] **T28.** `crates/syntrix-testkit/src/lib.rs`: Remove `make_invite_ticket()`, add `TestNode`:
  ```
  struct TestNode {
      secret: SecretKey,
      endpoint: Endpoint,
      gossip: Gossip,
      indexer: Arc<RelationalEngine>,
      registry: Arc<RwLock<NamespaceRegistry>>,
      topics: HashMap<String, GossipTopic>,
  }
  impl TestNode {
      async fn new() -> Self
      async fn create_org(&mut self, name: &str) -> TopicId
      async fn join_org(&mut self, name: &str, topic_id: TopicId, role: &str, admin_addr: EndpointAddr)
      async fn commit_event(&self, org: &str, event_type: &str, payload: Value)
      fn query_entity(&self, org: &str, entity: &str, filters: Vec<QueryFilter>) -> Vec<Value>
      fn get_document(&self, org: &str, entity: &str, doc_id: &str) -> Option<Value>
  }
  ```
- [ ] **T29.** Write E2E test: `two_peers_sync_event` — Admin writes customer.created, client receives via gossip and indexes.
- [ ] **T30.** Write E2E test: `write_permission_enforced` — Peer with role "sales" publishes payroll.created, receiver rejects (no can_write for payroll).
- [ ] **T31.** Write E2E test: `catch_up_on_join` — Client joins late, admin streams historical events via catch-up P2P, client has all events.
- [ ] **T32.** Write E2E test: `hlc_conflict_resolution` — Two peers write concurrently to same doc_id, higher HLC wins.
- [ ] **T33.** Write E2E test: `multi_org_isolation` — Peer in org A does not receive events from org B's topic.

### Phase 8: Frontend changes

- [ ] **T34.** `packages/syntrix-ui/src/collections/iroh-adapter.ts`: Minor updates:
  - `sync_pull` call unchanged (same Tauri command signature)
  - `commit_event` call unchanged
  - Remove `listen("data-changed")` — replaced by entity_changed events
  - Ensure `SyncConfig.sync` uses updated backend responses
- [ ] **T35.** Rename `iroh-adapter.ts` → `syntrix-adapter.ts` (cosmetic, reflects no-iroh-docs reality).

### Phase 9: Documentation

- [ ] **T36.** `apps/docs/.../arquitectura/sincronizacion.md`: Rewrite — replace all iroh-docs references with gossip + redb. Update mermaid diagrams.
- [ ] **T37.** `apps/docs/.../arquitectura/namespaces.md`: Rewrite — namespaces become entities; permission enforcement moves from accept_cb to app-level per-event validation.
- [ ] **T38.** `apps/docs/.../arquitectura/database.md`: Update — replace "Capa 1: Iroh Docs Event Log" with "Capa 1: redb EVENT_LOG + Gossip Broadcast".
- [ ] **T39.** `apps/docs/.../referencia/esquemas.md`: Remove namespace references, update permission section.
- [ ] **T40.** `apps/docs/.../referencia/rustdoc-iroh.md`: Archive or delete.

### Phase 10: Cleanup and validation

- [ ] **T41.** Remove `data_dir.join("docs")` directory creation from both apps — no longer needed without `Docs::persistent()`.
- [ ] **T42.** Remove `data_dir.join("blobs")` directory creation — no longer needed without `FsStore`.
- [ ] **T43.** Update `justfile`: Remove `IROH_DATA_DIR` from `client-2`, `admin-2`, `remote-admin-2`, `remote-client-2`. No longer needed.
- [ ] **T44.** Run `cargo check --workspace` — verify all crates and apps compile.
- [ ] **T45.** Run `cargo test -p syntrix-testkit` — verify 5 E2E tests pass.
- [ ] **T46.** Run `just admin && just client` — smoke test Tauri apps start correctly.

## Files Summary

| Action | Files |
|---|---|
| **Delete** | `accept.rs`, `capabilities.rs`, `rustdoc-iroh.md` |
| **Create** | `gossip.rs`, `catchup.rs`, `plans/1782758526139-remove-iroh-docs-gossip-sync.md` |
| **Rewrite** | `sync.rs` (core), `heartbeat.rs`, `identity.rs` ×2, `events.rs`, `sync.rs` (client), `audit.rs` ×2, `admin.rs`, `invite.rs`, `testkit/lib.rs`, `sincronizacion.md`, `namespaces.md` |
| **Update** | `registry.rs`, `lib.rs` (core), `lib.rs` (client), `lib.rs` (admin), `indexes.rs`, `iroh-adapter.ts`, `database.md`, `esquemas.md`, 4× `Cargo.toml`, `justfile` |

## Risks

- **TopicId collision**: UUID [u8; 32] makes collision astronomically unlikely. Deterministic hash-based IDs are worse (collision risk between admins with same org name).
- **Gossip message loss**: iroh-gossip uses broadcast to all peers. If a peer is offline, it catches up via point-to-point on reconnect. No missed events if at least one peer is online.
- **Catch-up bandwidth**: Full event log replay for new peers could be large. Mitigation: periodic snapshots (future enhancement). For dev phase, acceptable.
- **Permission validation gap**: Current accept_cb only checks is_device_active. New model validates sender's role.can_write per entity on every received event — strictly more secure.

## Validation

1. `cargo test -p syntrix-testkit` — 5 headless E2E tests validate gossip sync, permissions, catch-up, HLC conflicts, multi-org isolation.
2. `cargo check --workspace` — compilation across all crates.
3. `just admin && just client` — Tauri apps start and connect.
4. `just test-rust` — existing unit tests still pass.
5. `pnpm lint` — frontend lint passes.
