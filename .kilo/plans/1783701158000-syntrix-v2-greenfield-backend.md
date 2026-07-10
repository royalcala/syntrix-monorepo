# Plan: Syntrix v2 — Greenfield backend (Zenoh + SQLite) con contrato IPC preservado

> **Estado**: Dirección aprobada. **Proyecto nuevo** (greenfield del backend/servicios), **portando** el frontend React actual sin reescribirlo, preservando el contrato Tauri IPC.
> **Fecha**: 2026-07-10.
> **Relacionado / supersede**: `1783697037000-zenoh-rearchitecture-mobile-iot.md` (mantiene el análisis y las decisiones de stack; este plan **reemplaza su enfoque "modificar en sitio"** por greenfield). ADRs: `1783695719137-adr-libp2p-vs-zenoh.md`, `1783695719135-adr-libp2p-vs-headscale.md`. `main` (repo actual) sigue despachando hasta paridad.
> **Ámbito**: proyecto nuevo. Backend/servicios desde cero; frontend y reglas de negocio portados.

## 0. Enfoque

El core cambia entero (transporte libp2p→Zenoh, sync CDC→replicación Zenoh, DB Turso→SQLite) y la topología se invierte (malla→hub-and-spoke). Envolver lo viejo detrás de adaptadores es trabajo desechable. Por eso: **construir el backend/servicios en un proyecto nuevo**, y **portar** lo maduro y transporte-agnóstico.

La palanca que lo hace seguro: **el frontend no habla con libp2p ni Turso, habla con comandos y eventos Tauri IPC**. Si el contrato IPC se congela y el backend nuevo lo implementa, el frontend se porta sin reescritura funcional. G0 debe resolver explícitamente los pocos calls heredados que ya están rotos o no pertenecen al contrato (`upsert_entity` y `share_org`) antes de prometer una copia byte-a-byte.

## 1. Decisiones firmes (heredadas, ya resueltas)

- **Transporte**: Eclipse Zenoh v1.9.x (modos peer/client/router).
- **Motor de sync**: replicación Zenoh (storage-manager + backend custom sobre `Store`), anti-entropy por fingerprints, HLC nativo. **Sin CDC.**
- **DB**: SQLite vía `rusqlite` (feature `bundled`, FTS5), detrás de un trait `Store`.
- **Identidad**: Ed25519 a nivel app para firmar/validar autor de fila + permisos por rol (Zenoh no da identidad de autor por-fila; su ACL/TLS es defensa en profundidad).
- **Modelo núcleo**: **espacio de keys jerárquico** `syntrix/org/{org}/{modulo}/{entidad}/{doc}[/{sub}/{line}]`; **permisos = grants de key-expression** (DEC-8); **granularidad per-fila** (DEC-7 revisada). Ver §1.5.
- **Topología**: hub-and-spoke. **Ancla headless** por org (router Zenoh + store replicado autoritativo + autoridad admin). **No hay relay separado** — el ancla lo subsume.
- **Apps**: **una app unificada role-aware** (empleado + gobierno admin) + **servicio ancla headless**. Se depreca la app admin standalone.
- **`main`** (repo actual) se mantiene como producto vivo y **referencia** de reglas de negocio/casos-borde hasta el cutover.

## 1.5 Modelo de dominio y multi-org (transversal — lo que v2 DEBE preservar)

Verificado en el código actual. Este es el modelo que el greenfield debe reproducir de punta a punta; es donde más fácil se pierden casos-borde.

### Orgs (unidad de aislamiento)
- Un dispositivo pertenece a **múltiples orgs** simultáneamente (`orgs: HashMap`, `active_org`, `orgs.json`). El rol del nodo **difiere por org**.
- **Una sola DB por dispositivo** compartida entre orgs, con columna **`org_id`** en cada tabla; `NamespaceRegistry` keyed por `OrgId` (`crates/syntrix-core/src/registry.rs`).
- Mapeo v2: **key por org** `syntrix/org/{org_id}/**`; **replicación por scope de org** (cada org converge aislada); el storage backend parsea `org_id` de la key y escribe con `org_id` scoping. `active_org` es **solo UI**: el backend sincroniza **todas** las orgs.
- **Ancla multi-org**: un proceso ancla sirve N orgs (scope + config de replicación por org). Un dispositivo en orgs de anclas distintas mantiene una sesión Zenoh con múltiples endpoints (Zenoh multiplexa).

### Espacio de keys jerárquico (DEC-8 — modelo núcleo de v2)
Todo el dominio se direcciona como un árbol de key expressions; **entidad, jerarquía padre/hijo y permisos son el mismo path**:
```
syntrix/org/{org}/{modulo}/{entidad}/{doc_id}[/{subentidad}/{line_id}]
   ej: syntrix/org/acme/core/invoices/{id}/items/{line}
```
- **Módulo por defecto `core`**: siempre se incluye el nivel `{modulo}`. Las entidades actuales caen en `core` → `syntrix/org/{org}/core/{entidad}/...`. Evita migración masiva de URIs en Zenoh cuando el esquema crezca. Módulos adicionales (`compras`, `ventas`...) aparecen cuando el schema Drizzle los declare.
- **Storage sigue relacional**: la jerarquía es la capa de direccionamiento/permiso/sync; el `Store` (SQLite) mantiene tablas planas y **mapea path↔(tabla, fila)** para conservar joins/FTS/reportes. NO se convierte en KV.
- **Queryables por subárbol**: catch-up/scoped-sync = `subscribe`/`get` a un subárbol (`get .../compras/**`). La réplica scoped de un rol = los subárboles que sus grants conceden.

### Dispositivos, roles, permisos (DEC-8 — permisos como grants de key-expr)
- **Device**: `node_id` (pubkey Ed25519), `role`, `active`, `person`, `name` — **por org**.
- **Role = conjunto de grants de key-expression** (read/write) en vez de listas planas. Retro-compatible: `can_open:[invoices]` ≡ grant read `.../invoices/**`; `*` ≡ `.../**`. Un grant a nivel módulo (`.../compras/**`) cubre sus entidades e hijas. Defaults: `admin`(`.../**`), `sales`, `contabilidad`.
- **Doble aplicación**: la app es autoritativa (valida cada escritura por autor contra los grants del rol); el **ACL de Zenoh** (key-expr matching) es defensa en profundidad en el transporte.
- **Roster (`members`/`roles`)**: mapeo `node_id → role` + los grants del rol, por org. Base de la **validación por autor**: una escritura remota se acepta solo si el path de la key cae dentro de un grant `write` del rol del autor.
- **Autoridad**: el ancla (admin) funda la org (`create_org` registra el device admin) y es autoritativo del roster. Las **keys de roster/roles** (`syntrix/org/{org}/roster/**`, `.../roles/**`) solo las escribe la **llave admin**; los demás las validan contra esa firma. Revocación = baja de device (`update_device active=false`) + corte de sesión + revocar grant.

### Entidades y padre/hijo (= "namespaces", término legacy)
- "namespace" en el código = **entidad**. 6 hoy: `customers, suppliers, products, invoices, orders, payroll`.
- **Padre/hijo = niveles del path**: `invoices/{id}/items/{line}` (hoy `invoice_items`), `orders/{id}/items/{line}`. El grant del padre cubre a los hijos (prefijo de path).
- Columnas de sync en cada fila: `org_id, doc_id, change_time (HLC/LWW), node_id (autor)`. Esquema desde Drizzle (`packages/shared-drizzle` → `schema.json`); el backend lo consume genéricamente.

### DEC-7 (revisada) — Granularidad = per-fila / jerárquica
- **key = fila**: header en `.../{entidad}/{doc_id}`, cada línea hija en `.../{entidad}/{doc_id}/{subentidad}/{line_id}`. **LWW por fila** (editar líneas distintas del mismo documento no se pisa).
- **Desempate LWW determinista**: orden total de `uhlc::Timestamp` = (tiempo NTP64, contador lógico, **ID del HLC**). A HLC igual gana el ID mayor → convergencia estricta en todas las réplicas. (El `node_id` Ed25519 del autor es aparte, va en el attachment; no participa del desempate salvo que se use como ID del HLC.)
- **Forward-compat (columnas nuevas en entidad conocida)**: el **valor Zenoh crudo es autoritativo para la replicación**; las columnas SQL son una **proyección derivada**. Si un nodo recibe una fila con `schema_version`/columnas del futuro que no sabe proyectar, **guarda el valor crudo tal cual** (no lo descarta) para no perder datos ni romper la anti-entropy; proyecta lo que conoce y deja el resto opaco.
- **Forward-compat (entidad/tabla completamente nueva)**: si un nodo recibe una key `syntrix/org/{org}/core/{entidad_nueva}/...` para la que no tiene tabla en su schema, **almacena el valor crudo en una tabla overflow `_raw_unresolved`** keyed por la key-expr completa, sin descartarlo. Cuando la app se actualice y conozca la entidad, la migración de schema proyecta las filas acumuladas. **Corrige** el comportamiento actual (que descartaba cualquier fila de entidad desconocida).
- **Borrado de documento** = wildcard-delete del subárbol (`delete .../{doc_id}/**`), nativo en Zenoh (tombstones de subárbol + `garbage_collection`). Sin cascada manual.
- **Reconstruir documento** = query del subárbol (`get .../{doc_id}/**`); el `Store` lo proyecta a tabla padre + hijas.
- **Edge de consistencia**: un subscriber puede ver el header antes que sus líneas; la UI lee el documento como subárbol y tolera líneas que llegan un instante después (o marca "cargando líneas").

### Auditoría (`event_log`)
- `event_log` es **tabla de auditoría durable local** (Decisión 5): una fila por cambio (`entity/change_type/doc_id/row_image/change_time/node_id`), **no** transporte de sync. Se escribe al aplicar cambios; `audit_query`/`run_sql` la leen. Vive en el `Store` (SQLite); **no se replica** por Zenoh.

### IA org + permiso-aware
- `syntrix-ai` es **org-aware y permission-aware**: sus tools (`check_read_access(org,entity)`, `query_entity(org,...)`, `search_entity(org,...)`) van scoped por `org_id` y respetan `can_open`. v2 debe preservar ese scoping: los tools pasan por el mismo `Store` + checks de permiso.

**Fuera de alcance v2 (nota de futuro)**: **interconexión cross-org** (redes de franquicias / 100+ colectivos que comparten datos entre sí, `vision.md:28`). El modelo por-org aísla; el puente cross-org (nodo miembro de varias orgs que reexpone datos, o federación de anclas) es track posterior.

## 1.6 Aprovechamiento nativo de Zenoh (verificado en el fork)

En vez de reimplementar, se usan primitivas nativas de Zenoh:

- **DEC-9 — Convergencia por capa** (decidido):
  - **Hoja (móvil) = storage replica scoped**: storage-manager + replication sobre los **subárboles que sus grants conceden** → anti-entropy robusto tras offline largo.
  - **+ AdvancedPublisher/Subscriber** (`.history()` + `.recovery()`) para entrega **live** de baja latencia y recuperación de huecos cortos.
  - **Ancla↔ancla = replication** (anti-entropy) para HA/multi-ancla.
- **DEC-10 — Primitivas nativas**:
  - **Presencia = Liveliness tokens** (`session.liveliness()`), NO heartbeats custom. Alimenta `get_sync_info`/`sync_status`.
  - **Live + catch-up + recovery = AdvancedSubscriber** (unifica lo que hoy son `live_subscribe` + `get_updates_since` + catch-up).
  - **Autor + firma Ed25519 = Sample Attachment** (`sample.attachment`), separado del payload/row.
  - **Matching de grants (DEC-8) = `zenoh-keyexpr` `includes()`/`intersects()`** (reusar, no escribir matcher propio).
- **Capacidad (no bloqueante): REST plugin** en el ancla (HTTP GET/PUT sobre el keyspace) → habilita el módulo **"Puentes"** de la visión (SAT/CFDI, bancos, webhooks, APIs externas) y una web admin opcional, sin infra extra.
- **Transportes disponibles por deployment**: QUIC/TCP/TLS (base), **WS** (browser), **serial/vsock** (IoT), **SHM** (intra-host multiproceso).
- **Bindings (repos aparte)**: `zenoh-pico` (MCU, ya en G9), `zenoh-ts` (TS — el frontend *podría* hablar Zenoh directo; se descarta para preservar el contrato IPC), `zenoh-c/cpp/kotlin/java`.

## 2. El contrato IPC a congelar (la frontera del proyecto)

El backend nuevo DEBE implementar estos comandos con **mismos nombres, args y forma de retorno** para portar el frontend intacto. Catalogado del código actual:

**Datos / entidades / búsqueda:**
`commit_event`, `query_entity`, `query_entity_advanced`, `search_entity`, `drizzle_execute`, `get_schema_registry`, `check_entity_access`, `audit_query`, `run_sql`, `list_saved_views`, `create_saved_view`, `delete_saved_view`, `seed_dev_data`, `upsert_entity` (adaptador de compatibilidad temporal para la cola IA)

**Live / sync:**
`live_subscribe`, `live_unsubscribe`, `get_updates_since`, `get_sync_info`, `sync_status`, `network_status`

**Org / identidad / enrolamiento:**
`get_node_id`, `list_orgs`, `set_active_org`, `create_org`, `join_org`, `enroll_org`, `get_invites`, `get_invite_info`, `send_invite`, `get_endpoint_addr`, `get_pending_enrollments`, `approve_enrollment`, `reject_enrollment`

**Gobierno (devices/roles):**
`add_device`, `update_device`, `list_devices`, `create_role`, `update_role`, `list_roles`

**Logs / IA:**
`query_logs`, `summarize_logs`, `start_tail_logs`, `ai_chat`, `ai_status`

**Compatibilidad heredada a decidir en G0:** `share_org` aparece invocado por la UI admin actual, pero no está registrado por su backend actual. Se implementa como wrapper de `send_invite` o se elimina/refactoriza esa pantalla **antes** de congelar el snapshot; no se declara implícitamente soportado.

Notas de preservación (cambia la implementación, NO el contrato):
- `commit_event` → escribe en `Store` (proyección a columnas) + firma Ed25519 (en **attachment**) + `put` a Zenoh.
- `get_updates_since` / `live_subscribe` / `live_unsubscribe` → **AdvancedSubscriber** (history+recovery) + change-stream del `Store` (hooks SQLite), no `turso_cdc`.
- `get_sync_info` / `sync_status` → **liveliness tokens** (presencia) + estado de replicación Zenoh (reemplaza heartbeats).
- `search_entity` → **FTS5** en vez de `fts_match`/`fts_score` escalares.
- `drizzle_execute` → SQL dialecto SQLite (Drizzle ya lo genera).
- `send_invite` / `enroll_org` / `get_pending_enrollments` / `approve_enrollment` / `reject_enrollment` → sobre query firmada Zenoh `syntrix/org/{id}/admin-control` (el flujo de enrolamiento inverso ya existe en el contrato actual).
- La app unificada expone la **unión** de los command sets de client + admin; los de gobierno se gatean por rol admin.
- El contrato incluye también los **eventos Tauri y sus payloads**: `entity_changed`, `live_update`, `invite-received`, `log_event`; los nombres de argumentos serializados (camelCase de Tauri) y la forma exacta de errores son parte del snapshot.

## 2b. Seguridad y robustez (incorporado de revisión externa)

- **Bootstrap de confianza (QR)**: el QR de enrolamiento contiene la **pubkey Ed25519 del ancla**, el **endpoint**, el pin del certificado TLS (SPKI/fingerprint) **o una atestación Ed25519 firmada que lo vincule al certificado**, y un **token/secreto de enrolamiento de un solo uso**. El cliente verifica el enlace antes de enviar credenciales. Los tokens se guardan hasheados, con TTL, uso atómico único y rate-limit; la pubkey Ed25519 por sí sola no pinnea el canal TLS de Zenoh.
- **Protección DoS del ancla**: antes del chequeo Ed25519, aplicar **rate-limiting + límite de tamaño de payload** a nivel Zenoh (`low_pass_filter` `size_limit`, reservation/circuit rate limiters) para que un atacante no sature el storage con basura.
- **Suscripción gruesa + filtro fino local (escala de grants)**: suscribir/replicar a nivel **módulo/entidad** (key-expr simples, enrutamiento barato); los permisos finos (campo/fila) se aplican **localmente en lectura/escritura**, no como cientos de key-expr granulares en la suscripción. Evita penalizar el enrutamiento de Zenoh.
- **Límite de confidencialidad de los grants**: un filtro local **no es control de acceso** si la hoja ya recibió el payload: un dispositivo comprometido puede leer SQLite/crudo. Por tanto, la réplica gruesa solo puede abarcar datos que el rol está autorizado a conocer completos. Si el producto requiere grants por fila/campo con confidencialidad real, v2 debe usar scopes separados suficientemente gruesos para no filtrar datos indebidos, o cifrado/selective disclosure; no se puede resolver solo en la UI.
- **Forward-compat**: ver DEC-7 (valor crudo autoritativo; columnas desconocidas se preservan opacas, no se descartan).
- **Contenido firmado Ed25519 (especificación precisa)**: la firma en el `sample.attachment` vincula todos los parámetros críticos del mensaje para evitar replay, path-swap y tampering de timestamp:
  ```
  sign(key_expr_bytes || payload_blake3_hash || hlc_timestamp_bytes || operation_tag)
  ```
  Donde `operation_tag` = `put` (0x01) o `delete` (0x02). Esto fija la firma a un **path específico, contenido específico, momento HLC específico y operación específica**. Sin esto, un atacante que capture un sample válido puede moverlo a otro path o modificar el timestamp.
- **Envelope autenticado, no attachment como única prueba**: el trait `zenoh_backend_traits::Storage` recibe `key/payload/encoding/timestamp`, pero **no recibe el attachment**. Por ello el valor almacenado/replicado debe ser un envelope canónico versionado que contenga `author_node_id`, HLC, operación, `payload_hash`, firma y el row image (la firma cubre el mismo preimage indicado arriba). El attachment queda como optimización para el live path; el backend valida el envelope antes de proyectar. Un `delete` externo sin envelope no se acepta.
- **Deletes jerárquicos autenticables**: no se usa `delete .../{doc}/**` como operación remota de negocio mientras el backend no pueda verificar su autor. G0 valida un diseño de tombstone firmado: por lo menos tombstone del documento + tombstones por fila conocida, y una regla de proyección que suprima hijos con HLC $\leq$ al tombstone del padre. Solo tras un spike que pruebe que Zenoh preserva y replica esa semántica se permite usar wildcard-delete como optimización interna.
- **Clock/replay policy**: cada envelope lleva `op_id` determinista (hash del envelope firmado) y se deduplica antes de agregar `event_log`. El ancla limita el skew HLC futuro, mantiene watermarks por autor y rechaza timestamps incompatibles; la política para escrituras offline firmadas antes de una revocación se define explícitamente (recomendación: aceptarlas solo si HLC $\leq$ `revoked_at` y dentro del skew permitido).
- **Bloqueo de Foreign Keys en SQLite**: `PRAGMA foreign_keys = OFF` por defecto en el `Store`. Las FKs son **incompatibles con consistencia eventual**: en anti-entropy y live, una línea hija puede llegar antes que su header, produciendo errores de FK en el `INSERT`. La **integridad referencial la garantiza la jerarquía de keys de Zenoh** (`.../{doc_id}/items/{line_id}` implica que el padre existe lógicamente) más la validación de la app. El schema Drizzle se genera sin FKs.
- **Concurrencia del Store (single-writer SQLite)**: múltiples fuentes de escritura concurrentes (IPC `commit_event`, callbacks de AdvancedSubscriber, storage backend `Storage::put`) comparten una única DB. La impl SQLite usa un **actor pattern: un solo task/thread procesa un canal `mpsc` de escrituras**, serializando todos los writes. El trait `Store` define las operaciones como async con serialización interna garantizada por la impl. Evita `SQLITE_BUSY` y deadlocks.
- **Política de GC de tombstones**: `garbage_collection` de Zenoh configurado con ventana de **60 días** (cubre offline largo de un móvil típico; 30–90 días) y `lifespan` acorde. Si un nodo regresa tras la ventana de GC: la réplica autoritativa del ancla ya no tiene la key → la anti-entropy de replicación alinea y elimina la key local. **Test específico en G4**: "nodo offline más tiempo que la ventana de GC, ¿converge correctamente al reconectar?".
- **`drizzle_execute` como read-only scoped**: solo ejecuta queries de **solo lectura** (validación de sintaxis: sin INSERT/UPDATE/DELETE/DROP/ALTER). Además, **inyecta automáticamente** el filtro `org_id = {active_org}` o valida que la query resultante no acceda a filas de otras orgs. Toda escritura pasa exclusivamente por `commit_event` (validación Ed25519 + grants + `event_log`). Una conexión SQLite separada sin permisos de escritura para esta ruta.
- **Scoping SQL robusto**: no basta concatenar un `WHERE`: CTEs, joins, aliases y subqueries lo pueden evadir. `drizzle_execute` usa un parser AST con allowlist de tablas/operaciones y reescribe cada tabla multi-org a una vista filtrada por `org_id`, o se sustituye por un query API tipado. La conexión read-only es una segunda barrera, no una solución de aislamiento.

## 3. Estructura del proyecto nuevo

```
syntrix/ (proyecto nuevo)
├── apps/
│   ├── app/                    ← Tauri: frontend React PORTADO + backend Rust nuevo (todos los comandos IPC)
│   └── anchor/                 ← servicio headless: router Zenoh + storage replicado + autoridad admin + REST plugin (opcional)
├── crates/
│   ├── syntrix-ipc/            ← contrato IPC + handlers (compartidos por app y anchor headless)
│   ├── syntrix-domain/         ← PORTADO: permisos (grants key-expr), upcasters, HLC, proyección path↔fila, folios, schema registry
│   ├── syntrix-store/          ← trait Store
│   ├── syntrix-store-sqlite/   ← rusqlite (bundled) + FTS5 + migraciones
│   ├── syntrix-transport/      ← trait SyncTransport
│   ├── syntrix-zenoh/          ← impl Zenoh: sesión, pub/sub, query, liveliness, AdvancedSub, storage backend + replication
│   ├── syntrix-identity/       ← Ed25519: keypair, firma/validación de autor (attachment)
│   ├── syntrix-ai/             ← PORTADO
│   └── syntrix-logging/        ← PORTADO
└── packages/
    ├── syntrix-ui/             ← PORTADO (design system)
    └── shared-drizzle/         ← PORTADO (schema/entidades)
```

## 4. Portar vs construir

**PORTAR tal cual (copiar, no reescribir):**
- Frontend React completo (EntityGrid, DetailPanel, FieldRegistry, TanStack, shadcn, Command Palette).
- `packages/syntrix-ui`, `packages/shared-drizzle` (schema/entidades Drizzle).
- `syntrix-ai`, `syntrix-logging`.

**PORTAR como referencia + limpiar (misma lógica, crate nuevo `syntrix-domain`):**
- Permisos por rol/autor (`can_open`/`can_write` por entidad, wildcard `*`, hijo hereda entidad padre; validación por `node_id`).
- Roster (`members`/`roles`) + validación por autor de eventos remotos.
- Upcasters de `schema_version`, HLC/LWW por documento, proyección documento→padre/hijo, folios, schema registry (entidad/namespace + aliasing).
- Auditoría `event_log` (escritura al aplicar; lectura por `audit_query`/`run_sql`).
- Scoping org+permiso de los tools de `syntrix-ai` (`check_read_access`, `query/search` por `org_id`).

**CONSTRUIR nuevo:**
- `syntrix-store` + `syntrix-store-sqlite`, `syntrix-transport` + `syntrix-zenoh`, backend de storage Zenoh sobre `Store`, `syntrix-identity`, `syntrix-ipc`, la app unificada (cascarón backend) y el `anchor`.

## 5. Fases (ordenadas)

### G0 — Repo nuevo aislado + portar assets + congelar contrato
- **Ship estable primero**: preservar el estado actual del worktree en la branch **`syntrix-v02`** (release estable). No se toca más.
- **Crear repo nuevo `syntrix-v03`** (aislado del monorepo actual). Razón: `pnpm` comparte `node_modules`/`packages` entre worktrees del monorepo; editar `packages/shared-drizzle` en un worktree rompería el checkout de `syntrix-v02`. El repo nuevo aísla pnpm y Cargo. (El `target/` de Cargo ya es local por worktree, pero pnpm no.)
- **Copiar (copia única, no submodule)** al repo nuevo: frontend React (`apps/*/src`), `packages/syntrix-ui`, `packages/shared-drizzle`, y crates portables `syntrix-ai`, `syntrix-logging`.
- **Cargo + pnpm nacen limpios**: solo `zenoh` + `rusqlite` (sin `libp2p`, sin `turso_core`).
- **Congelar el catálogo de comandos y eventos IPC** (§2) como contrato versionado: extraer `generate_handler!`, tipos serializados y todos los `invoke()`/`listen()` del frontend a un JSON Schema/fixtures de args, retornos, errores y eventos. Un test debe fallar si el frontend portado invoca algo ausente. Resolver antes del freeze `upsert_entity` (wrapper a `commit_event` o adaptador transitorio) y `share_org` (hoy invocado pero no implementado).
- **Spike bloqueante de Zenoh (antes de construir G1–G4)**: con la versión exacta fijada, probar en un binario mínimo que (a) `Storage::put/delete` no expone attachments, (b) envelopes firmados sobreviven `storage-manager` + replication, (c) tombstones firmados convergen en scopes hoja $\subset$ ancla, y (d) el backend puede rechazar samples sin envelope antes de proyectar SQLite. Si falla, ajustar el diseño de ingestión; no empezar la migración sobre el supuesto de validar attachments en `Storage`.

### G1 — `Store` SQLite (rutas de lectura del frontend)
- `syntrix-store` (trait) + `syntrix-store-sqlite`: **actor pattern de escritura serializada** (un task procesa un canal `mpsc` de writes; todas las fuentes concurrentes —IPC, AdvancedSubscriber, storage backend— pasan por el mismo canal). `PRAGMA foreign_keys = OFF` (integridad referencial por jerarquía de keys, no por FK). Migraciones desde Drizzle **sin FKs**, **mapeo path↔(tabla, fila)** (DEC-7 per-fila), columnas de sync, FTS5.
- Reconstrucción de documento = query de subárbol (`get .../{doc}/**`) proyectada a padre+hijas.
- Implementar `query_entity`, `query_entity_advanced`, `search_entity`, `drizzle_execute`, `get_schema_registry`, `audit_query`/`run_sql` (leen `event_log` local), saved views.
- **Meta**: el frontend renderiza grids/búsqueda contra SQLite; facturas con líneas hijas se leen como subárbol; writes concurrentes no producen `SQLITE_BUSY`.

### G2 — `syntrix-domain` + escritura + autorización
- Portar: **permisos como grants de key-expr (DEC-8)** por rol (retro-compat con `can_open`/`can_write`; hijo cubierto por prefijo de path; wildcard `.../**`), roster (`members`/`roles`), **validación por autor** (path de la key ∈ grant write del rol), upcasters de `schema_version`, HLC/**LWW por fila**, folios.
- `syntrix-identity` (Ed25519). Implementar `commit_event` (valida grant propio → firma Ed25519 vinculando `key_expr||payload_hash||hlc||op_tag` → `Store::put` de las filas afectadas → escribe auditoría en `event_log`) y `check_entity_access`.
- **`drizzle_execute` como read-only y org-scoped**: solo SELECT (sin INSERT/UPDATE/DELETE/DROP/ALTER, validado en el backend). **Auto-inyecta `WHERE org_id = {active_org}`** o valida que la query no acceda a otras orgs. Conexión SQLite separada sin permisos de escritura. Toda escritura pasa por `commit_event`.
- **Meta**: CRUD local completo con permisos, auditoría y reactividad optimista, sin red; `drizzle_execute` no es escape hatch de escritura ni de cross-org.

### G3 — Transporte Zenoh (LAN peer) + validación por autor en apply
- `syntrix-transport` (trait) + `syntrix-zenoh` (sesión, pub/sub, query, **liveliness** para presencia).
- Escritura: crear el **envelope firmado canónico** en el payload; puede duplicar `node_id`/firma en `Sample Attachment` únicamente para acelerar live, pero la validez no depende de él.
- Al recibir un sample: **validar envelope Ed25519 + grant + skew HLC + deduplicación `op_id`** — el path de la key ∈ grant write del rol del autor, usando **`zenoh-keyexpr` `includes()`** — **antes** de escribir. Descartar deletes nativos no autenticados y samples no autorizados; escribir una única auditoría en `event_log`.
- Dos instancias en LAN se descubren (scouting) y propagan writes autorizados; presencia mutua vía liveliness.

### G4 — Replicación por capa + backend sobre `Store` + live
- Backend `zenoh_backend_traits::{Volume,Storage}` sobre `Store`; **el `put` valida el envelope canónico y mapea la key (fila) a (tabla, fila)**. El borrado de documento se materializa como tombstones firmados por fila/documento y la proyección aplica el tombstone padre; wildcard-delete queda restringido a una optimización interna demostrada en el spike de G0.
- **DEC-9 por capa**: cada nodo (hoja y ancla) es **storage replica** con storage-manager + replication sobre su scope (hoja = subárboles concedidos; ancla = org completa; interval/hot/warm/gc). **AdvancedPublisher/Subscriber** (`.history()`+`.recovery()`) para entrega live y recuperación de huecos.
- **Política de GC de tombstones**: ventana de **60 días** (`garbage_collection` `lifespan`) solo después de verificar que ancla y hoja retienen suficiente watermark/tombstone para impedir resurrecciones. Si un nodo regresa tras la ventana, la anti-entropy alinea con el ancla y la proyección no debe revivir una fila suprimida. **Test específico**: "nodo offline más tiempo que la ventana de GC, ¿converge correctamente al reconectar?".
- **Suscripción gruesa + filtro fino local** y **Forward-compat** como en G1–G3.
- `get_updates_since` / `live_subscribe` / `live_unsubscribe` → AdvancedSubscriber + change-stream del `Store`; `get_sync_info` / `sync_status` → liveliness + estado de replicación.
- **Meta**: anti-entropy real (nodo offline largo reconecta y converge); líneas hijas convergen aunque lleguen tras el header; nodos de distinta versión no divergen; GC no rompe convergencia de nodos offline largo.

### G5 — Ancla headless + roster + gobierno/enrolamiento + integraciones
- `apps/anchor`: router Zenoh + storage replicado autoritativo + endpoint público (QUIC/TCP/TLS), `--keypair-file`, `--data-dir`. **Multi-org capable**. **Ancla↔ancla = replication** para HA/multi-ancla.
- **Seguridad del ancla**: **rate-limiting + límite de payload** a nivel Zenoh (`low_pass_filter` `size_limit`, circuit rate limiters) **antes** del chequeo Ed25519 (previene saturación DoS). **QR de enrolamiento** contiene pubkey Ed25519, endpoint, pin TLS o una atestación de enlace Ed25519→certificado, y token de un solo uso hasheado/TTL → cliente verifica el canal antes del primer paquete autenticado.
- **Roster autoritativo**: las keys `syntrix/org/{org}/roster/**` y `.../roles/**` solo las escribe la **llave admin**; los nodos las validan contra esa firma y pueblan `members`/`roles` locales.
- Protocolo `syntrix/org/{id}/admin-control` (query firmada): `create_org` (funda org + device admin), `add_device`, `update_device` (baja/revocación), `create_role`, `update_role`, `send_invite`, `enroll_org`, `get_pending_enrollments`, `approve_enrollment`, `reject_enrollment`.
- Emparejamiento teléfono-admin ↔ ancla por QR; llave raíz solo en el ancla.
- **Recuperación y rotación**: definir en el primer deploy backup cifrado/restauración de la llave raíz y una operación firmada de rotación de llave admin/anchor. La pérdida de una llave raíz no puede dejar una org permanentemente sin capacidad de revocar o enrolar.
- **Opcional — REST plugin** en el ancla: expone el keyspace por HTTP para el módulo "Puentes" (SAT/CFDI, bancos, webhooks) y web admin opcional.

### G6 — App unificada role-aware
- Fusionar comandos client+admin; módulo "Gobierno" gateado por rol admin (control remoto del ancla).
- **UI de gobierno (grants)**: expone permisos como **checkboxes por módulo/entidad** (no grants crudos de key-expr); compila a key-expr al guardar (`compras/**`). Usuarios no ven jerarquías de base de datos.
- Clientes en modo **client** conectan outbound al ancla; **réplica scoped gruesa** (módulo/entidad, key-expr simples) con filtro fino de permisos local; `get` on-demand para lo demás.

### G7 — Móvil
- Targets Tauri Android/iOS; paths sandbox; SQLite `bundled` compila con toolchain C de Tauri mobile.
- Foreground-sync + throttling batería; presencia por **liveliness tokens**; push opt-in (APNs/FCM, dependencia central etiquetada).

### G8 — Paridad + cutover
- Suite E2E de paridad contra `main` (invite/enroll, sync multi-nodo, permisos, búsqueda, auditoría).
- Cuando v2 pase la suite: v2 se vuelve el producto; `main` queda archivado como referencia.

### G9 — IoT (post-cutover)
- Hojas MCU: `zenoh-pico` (C), sin DB local, publican a `syntrix/org/{id}/core/{entity}/**` (módulo `core` por defecto).
- Gateways SBC (Linux ARM): `syntrix-store-sqlite`. Permisos/entidades acotadas para nodos IoT.

## 6. Riesgos

| Riesgo | Mitigación |
|---|---|
| Rewrite pierde casos-borde del backend actual | `main` es la referencia viva; portar reglas a `syntrix-domain` con tests derivados del comportamiento actual. |
| Deriva del contrato IPC rompe el frontend portado | Congelar el catálogo (§2) con tests de contrato; el frontend no se toca. |
| APIs `unstable` de Zenoh (advanced pub/sub, replication) | Aceptado; fijar versión; aislar tras `SyncTransport`; tests de convergencia. |
| Sync depende de storage-manager replication **y** AdvancedSubscriber (ambos `unstable`) | Superficie unstable concentrada en `syntrix-zenoh`; suite de convergencia (offline largo, huecos, deletes) como gate de versión. |
| Alineación de replicación con scopes distintos (hoja ⊂ ancla) | La hoja replica un subárbol del scope del ancla; validar alineación de fingerprints en la intersección (test hoja-scoped ↔ ancla-full en G4). |
| Anti-entropy Zenoh: wildcard delete/tombstones/orden | Tests dedicados en G4; configurar `garbage_collection`. |
| Identidad: ACL Zenoh no es autor por-fila | Ed25519 app-level (`syntrix-identity`); ACL Zenoh solo defensa en profundidad. |
| Storage backend Zenoh escribe sin validar autor (bypass de permisos) | El `put` del backend corre validación por autor (firma + grant de key-expr) antes de escribir; tests de aislamiento por rol. |
| `Storage` de Zenoh no recibe attachment; la firma se pierde en el camino storage/replication | Envelope firmado dentro del payload canónico; attachment solo optimización. Spike bloqueante G0 y tests de rechazo antes de la proyección. |
| Roster no llega antes que los datos → autor no validable | El roster (keys admin) se replica/pide primero; si falta el rol del autor, encolar/reintentar en vez de aceptar. |
| Per-fila: subscriber ve header antes que sus líneas (documento parcial) | La UI lee el documento como subárbol (`get .../{doc}/**`) y tolera líneas que llegan un instante después; opcional marca "cargando líneas". |
| Per-fila: borrar documento debe eliminar todas sus líneas | Wildcard-delete de subárbol nativo de Zenoh (`.../{doc}/**`) + `garbage_collection`; test de borrado en G4. |
| Grants de key-expr mal formados abren/cierran de más | Retro-compat estricta (`[entidad]`≡`.../entidad/**`); tests de matching de grant vs key. |
| Réplica gruesa filtra localmente pero expone datos a una hoja sin derecho | El scope replicado es como mínimo el límite de confidencialidad; datos por fila/campo requieren scopes separados o cifrado/selective disclosure. |
| Forward-compat: nodo viejo descarta columnas nuevas → fingerprints divergen | **Valor Zenoh crudo es autoritativo** (DEC-7 revisada); columnas desconocidas opacas en crudo, no se descartan. Test nodo-viejo ↔ nodo-nuevo en G4. |
| Forward-compat: entidad nueva desconocida → path no proyectable, fila perdida | Tabla overflow `_raw_unresolved` keyed por key-expr; se proyecta al actualizar el schema. Test entidad-nueva ↔ nodo-viejo en G4. |
| Ancla sin endpoint público (NAT oficina) | VPS/port-forward; documentar. |
| QR pinnea una llave Ed25519 de aplicación pero no el canal TLS | QR incluye pin SPKI/TLS o atestación firmada de binding; tokens de enrolamiento son hash+TTL+uso único. |
| Pérdida de llave raíz del ancla bloquea gobierno/revocación | Backup cifrado, procedimiento de recuperación y rotación firmada probados en G5. |
| FK violadas por llegada fuera de orden (línea antes que header) | `PRAGMA foreign_keys = OFF`; integridad referencial por jerarquía de keys + validación app. Test FK-off en G1. |
| Contención de escritura concurrente en SQLite (`SQLITE_BUSY`) | Actor pattern de write serializado en el `Store` (G1); todas las fuentes de escritura pasan por un canal `mpsc`. |
| GC de tombstones: nodo offline > ventana de GC resucita datos borrados | Ventana de 60 días; test offline > GC en G4; anti-entropy alinea contra el ancla (si no tiene la key, la hoja la elimina). |
| `drizzle_execute` como escape hatch de escritura o cross-org | Read-only (solo SELECT, validado en backend) + auto-inyección de `WHERE org_id = ?`; conexión sin permisos de escritura. Test en G2. |
| Reescritura SQL por texto permite evadir el filtro de org | Parser AST + allowlist y vistas filtradas por org para cada tabla; tests con CTE/join/subquery/alias. |
| Firma Ed25519 no vincula path/timestamp → replay o path-swap | Firma vincula `(key_expr || payload_hash || hlc || op_tag)`; test de replay-cross-path rechazado en G3. |
| Writer autorizado adelanta HLC o reinyecta un evento y bloquea LWW/auditoría | Límite de skew/watermark en ancla y `op_id` deduplicado; definir semántica de escrituras previas a revocación. |
| v2 tarda en alcanzar paridad | `main` sigue despachando; cutover solo tras E2E verde. |

## 7. Validación

0. **Proptest de convergencia**: property-based testing (`proptest`): secuencias aleatorias de operaciones offline/online, reordenadas, sobre varias réplicas → **todas terminan con el mismo hash de DB** (LWW + anti-entropy correctos).
1. G1: frontend renderiza grids/búsqueda contra SQLite; facturas con líneas hijas reconstruidas como subárbol; `get_schema_registry`/`drizzle_execute` funcionan; **writes concurrentes serializados (sin `SQLITE_BUSY`)**; **FKs off, no error por línea antes que header**.
2. G2: CRUD local + permisos como grants de key-expr (incl. hijo cubierto por prefijo) + upcasters + auditoría en `event_log` (paridad con `main` sin red); **`drizzle_execute` solo SELECT + org-scoped; INSERT/UPDATE/DELETE rebotan**.
3. G3: sync LAN por Zenoh; envelope firmado canónico vinculando `(key_expr||hash||hlc||op_tag)`; **replay a otro path, timestamp modificado, sample sin envelope y duplicado de `op_id` son rechazados/no duplican auditoría**; presencia vía liveliness.
4. G4: anti-entropy (offline **largo**→converge); hoja-scoped ↔ ancla-full alinean; LWW per-fila; tombstones jerárquicos firmados; live+recovery por AdvancedSubscriber; forward-compat de **columnas nuevas** (opacas) y de **entidad nueva** (overflow `_raw_unresolved`, no se pierde ni diverge); **nodo offline > 60 días (ventana GC) converge al reconectar sin resucitar datos borrados**.
5. G5: **QR enlaza pubkey Ed25519 del ancla con TLS** → cliente verifica el canal y previene MitM; tokens son hash+TTL+uso único; **rate-limiting + límite de payload** rechazan basura antes del chequeo Ed25519; roster firmado por admin puebla `members`/`roles`; enrolamiento inverso, recuperación/rotación de root y gobierno remoto sin exponer la llave raíz.
6. G6: app unificada; rol admin ve gobierno; **réplica scoped gruesa (módulo/entidad) con filtro fino local**; grants se configuran por checkboxes (no key-expr crudo); IA respeta org + grants.
7. Multi-org: un dispositivo en 2+ orgs sincroniza cada org aislada (sin cruce); rol distinto por org respetado; un ancla sirviendo 2 orgs las mantiene separadas.
8. G7: app arranca y sincroniza en Android/iOS reales (SQLite bundled).
9. G8: E2E de paridad verde antes del cutover.

## 8. Decisiones abiertas (confirmar)

Las siguientes se marcan como **resueltas por revisión cruzada** (GEMINI); solo quedan abiertas las de **negocio/operación**:

**Resueltas (técnicas):**
- **Repo del proyecto nuevo**: repo independiente `syntrix-v03` (aislado del monorepo; ver G0).
- **Nombre/versión**: repo `syntrix-v03` durante desarrollo; de cara al usuario final es una actualización invisible. El actual se preserva en la branch `syntrix-v02` como release estable y referencia.
- **Replicación**: `History::Latest` (LWW; historia en `event_log`).
- **Persistencia multi-org**: una DB por dispositivo con `org_id` scoping.
- **Ancla multi-tenant**: diseñar multi-org desde el día 1; config permite single-org. Mismo binario, distinto deploy.
- **Módulos en el path**: **siempre** incluir el nivel `{modulo}`. Las entidades actuales caen en un módulo por defecto `core` → `syntrix/org/{org}/core/{entidad}/...`. Evita migración futura de URIs cuando el esquema crezca.
- **UI de grants**: checkboxes por módulo/entidad que compilan a key-expr al guardar (no grants crudos).

**Abiertas (negocio/operación):**

- **Ancla hosteada por Syntrix (SaaS) además de self-host**:
  - Recomendación: **Fase 1 (lanzamiento post-G8) solo self-host.** El cliente corre su propia ancla con soporte y documentación (minimiza objeciones de datos sensibles). **Fase 2 (crecimiento): "Ancla Gestionada" opcional.** Syntrix corre el mismo binario por él como suscripción mensual. Arquitectónicamente no cambia nada (el binario es idéntico): la decisión es solo de quién lo opera. Self-host = licencia perpetua/one-time; managed = suscripción recurrente.
  - **No bloquea G0–G8.** Resuelto para el diseño.

- **Un ancla por org al inicio vs multi-ancla/HA**:
  - Recomendación: **Una ancla por org al inicio. HA como feature premium posterior (post-cutover).** Razones: (1) Simplicidad operativa para PyMEs (5-50 dispositivos, una oficina). (2) Si el ancla se cae, los nodos hoja siguen operando offline con su réplica local; al reconectar, la anti-entropy reconverge. El ancla no es SPOF de operación, solo de sincronización — tolerable. (3) HA requiere resolver split-brain del roster, failover y consistencia del store autoritativo entre múltiples anclas, una superficie de bugs considerable que no es necesaria para el primer despliegue. (4) La base arquitectónica (ancla↔ancla replication, DEC-9) ya está diseñada; implementar failover es upsell futuro.
  - **No bloquea G0–G8.** Resuelto para el diseño.