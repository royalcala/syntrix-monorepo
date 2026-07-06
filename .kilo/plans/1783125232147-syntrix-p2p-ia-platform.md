# Syntrix — Plataforma P2P Local-First con IA Generativa

## 1. Estado Actual (Línea Base)

### Lo que ya existe en código

| Capa | Componente | Detalle |
|---|---|---|
| **Sync** | CDC vía Limbo + libp2p gossipsub | `syntrix-network::cdc`, batches de `CdcEvent`, `change_time` LWW |
| **DB** | turso_core (Limbo SQL) + Tantivy FTS | Schema-driven desde `schema.json`, Drizzle proxy (`drizzle_execute`), columnas tipadas |
| **Auth** | libp2p identity (ed25519 keypair) | Permisos `can_open`/`can_write` por rol, `check_read_access` en Rust |
| **Entidades** | 6: customers, products, invoices, orders, suppliers, payroll | Definidas en `packages/shared-drizzle/src/entities.ts` y `crates/syntrix-network/schema.json` |
| **Relaciones** | Padre-hijo: invoices→invoice_items, orders→order_items | FK implícitas: `invoices.customer_id`, `invoice_items.product_id` |
| **Frontend** | React + TanStack Table + shadcn/ui | `EntityGrid` + `DetailPanel` hardcodeados por entidad |
| **Admin** | `apps/admin/` con 8 pantallas | Devices, Roles, Orgs, Sync, Schemas, Audit, SQL, Logs |
| **Headless** | Modo `--headless` (stdin/stdout JSON) | Usado en tests binarios E2E |

### Admin App — Inventario de pantallas existentes

| Ruta | Pantalla | Componente | Se mantiene? |
|---|---|---|---|
| `/devices` | Dispositivos | `DevicesGridPage` | ✅ Extender (tipo `client-ia`) |
| `/roles` | Roles | `RolesGridPage` + `PermissionMatrix` | ✅ Sin cambios |
| `/orgs` | Organizaciones | `OrgsGridPage` | ✅ Sin cambios |
| `/sync` | Sincronización P2P | `SyncDetailsPage` | ✅ Extender (CDC pendientes) |
| `/schemas` | Esquemas | `SchemaExplorer` | ✅ Sin cambios |
| `/audit` | Auditoría | `AuditTrail` → `SqlConsole` | ✅ Sin cambios |
| `/sql` | Consola SQL | `SqlConsole` + `saved_views` | ✅ Sin cambios |
| `/logs` | Logs | `Logs` (tailing, filtros, gráficas) | ✅ Sin cambios |

### Client App — Estado actual

| Ruta | Contenido |
|---|---|
| `/customers` | `EntityGrid(customersEntity)` |
| `/invoices` | `EntityGrid(invoicesEntity)` |
| `/products` | `EntityGrid(productsEntity)` |
| `/orders` | `EntityGrid(ordersEntity)` |
| `/inbox` | Invitaciones P2P pendientes |
| `/orgs` | Mis organizaciones + dirección P2P |
| `/sync` | `SyncDetailsPage` |

---

## 2. Visión Completa

Syntrix no es un ERP — es una **plataforma de colaboración soberana** donde cada colectivo (empresa, familia, comunidad) tiene control absoluto sobre su información. La IA generativa construye la interfaz bajo demanda: el usuario habla, la IA responde con la vista que necesita.

### Fases

```
┌──────────────────────────────────────────────────────────────┐
│  FASE C: Hardware + Inter-org + IoT                          │
│  ┌─────────────────────┐  ┌───────────────────────────────┐  │
│  │ Fierros huecos      │  │ Comunicación P2P entre orgs   │  │
│  │ NixOS + auto-update │  │ Registry P2P de módulos       │  │
│  │ Tipos: router, iot  │  │ Factura proveedor → cliente   │  │
│  └─────────────────────┘  └───────────────────────────────┘  │
│                                                               │
│  FASE B: Sistema de Módulos                                  │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ ModuleDefinition: schema + vistas + reglas + permisos   │ │
│  │ IA extiende templates base por org                      │ │
│  │ Reglas de validación inmutables (contabilidad, CFDI)    │ │
│  │ Catálogo ampliado: chart, reports                       │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  FASE A: Client-IA + Shell Universal (ESTE PLAN)             │
│  ┌──────────────────┐  ┌──────────────────────────────────┐  │
│  │ Client-IA        │  │ Shell Universal (client app)     │  │
│  │ Peer headless    │  │ Voice → Vista                    │  │
│  │ Ollama + Rig     │  │ Catálogo 7 componentes           │  │
│  │ CDC queue fallback│ │ Mis Vistas / Org / Recientes     │  │
│  │ ModelRouter      │  │ Entidades actuales como fallback │  │
│  └──────────────────┘  └──────────────────────────────────┘  │
│                                                               │
│  CAPA BASE (ya existe):                                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ libp2p · Limbo CDC · FTS Tantivy · Permisos · Headless │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Decisiones de Arquitectura

| # | Decisión | Elección | Rationale |
|---|---|---|---|
| 1 | Transporte client↔IA | P2P request-response (libp2p) | Aprovecha stack existente. No nuevo protocolo. |
| 2 | Protocolo UI IA→client | Custom catalog + adjacency list | Inspirado en A2UI pero simplificado. A2UI asume server→client streaming con datos incluidos; nosotros solo enviamos ViewDefinition (datos ya locales). |
| 3 | Catálogo componentes (Fase A) | dataTable, metricCard, entityDetail, column, row, card, form | 7 componentes. Suficiente para reemplazar EntityGrid + DetailPanel. Charts deferidos a Fase B. |
| 4 | STT (speech-to-text) | Nativo del SO en client app | iOS: SFSpeechRecognizer, Android: SpeechRecognizer, Desktop: Web Speech API. Audio nunca sale del dispositivo. Client envía texto al client-ia. |
| 5 | Almacenamiento de vistas | Entidad P2P vía CDC (`view_definitions`) | Sincronizada a todos los peers. UI: Mis Vistas, Org, Recientes. Metadatos: tags, createdBy, usageCount. |
| 6 | IA genera | Todo: vistas consulta, forms mutación, módulos | La IA no solo lee — también genera interfaces para crear/editar datos. |
| 7 | Validaciones | Reglas P2P sincronizadas vía CDC | JSON declarativo (`field op value`). Reglas contables/fiscales inmutables (parte del módulo). |
| 8 | LLM engine | Ollama (daemon) + Rig (librería Rust) | Ollama maneja modelos, cuantización. Rig provee loop de tool-calling + multi-provider. |
| 9 | Modelos locales | granite3.2:2b, qwen2.5:3b, nous-hermes:8b | Seleccionados por ModelRouter según task. Benchmarks propios validan selección. |
| 10 | ModelRouter | Routing por task type | Vistas simples→granite2b, Tool-calling→qwen3b, Módulos complejos→hermes8b o cloud fallback. |
| 11 | Cloud fallback | Groq/DeepSeek vía Rig provider | Solo cuando modelo local falla o timeout. Opt-in, no default. |
| 12 | Client-ia discovery | Admin designa peer como `client-ia` (tipo) | Configuración se replica vía CDC. Client app lee de DB local qué peer es IA. Sin protocolo nuevo. |
| 13 | View matching | Tags semánticos generados por IA | Al crear vista: `tags: ["facturas", "mayo", "pendientes"]`. Al buscar: fuzzy match sobre tags. Embeddings deferidos a Fase B. |
| 14 | Entidades actuales | Coexistencia | Las 6 entidades hardcodeadas no se eliminan. Son fallback cuando no hay IA configurada. Nuevos módulos IA se agregan sin tocar las existentes. |
| 15 | WASM | No se adopta | No ejecutamos código externo. Cada org genera sus propios módulos. Sin necesidad de sandboxing. |
| 16 | Código client-ia | Mismo binario `syntrix-client --headless --ia` con feature flag | Reusa AppState, SqlEngine, network stack. Crate `syntrix-ai/` como librería. Sin binario separado. |
| 17 | Dispositivos IoT/red | Tipos `router`, `iot` documentados para Fase C | Extienden el modelo actual de devices (ya tienen node_id, role, active, name). |
| 18 | Validación IA offline | Cola CDC + retry multi-modelo | Si IA timeout: query se guarda como entidad P2P. CDC la entrega cuando IA vuelve online. Client muestra badge. |
| 19 | Permisos IA | Solo tools read-only + `save_view` | `check_read_access` en todas las tools. Sin acceso a internet (solo Ollama local). Sin commit_event para datos de negocio. |

---

## 4. Fase A — Client-IA + Shell Universal

### 4.1 Arquitectura del Client-IA

```
┌─ Binario syntrix-client --headless --ia ─────────────────────┐
│                                                                │
│  crates/syntrix-ai/                                            │
│  ├── src/lib.rs              ai_chat_impl (agent loop)         │
│  ├── src/tools.rs            query_entity, search_entity,      │
│  │                           get_schema, list_views, save_view │
│  ├── src/router.rs           ModelRouter (task→model)          │
│  └── src/bench.rs            Benchmark suite para modelos      │
│                                                                │
│  Rig (rig-core)                                                │
│  ├── Agent loop: msg → LLM → tool → result → LLM → final      │
│  ├── ModelProvider trait (Ollama, OpenAI, Groq)                │
│  └── Streaming: tokens via P2P event al client app             │
│                                                                │
│  Ollama (daemon externo)                                       │
│  ├── granite3.2:2b  → vistas simples                          │
│  ├── qwen2.5:3b     → tool-calling estructurado               │
│  ├── nous-hermes:8b → tool-calling avanzado                   │
│  └── API OpenAI-compatible en localhost:11434                  │
│                                                                │
│  Tauri commands (thin wrappers):                               │
│  ├── ai_chat(org_id, messages) → stream ai_chat_event          │
│  └── ai_status() → modelos disponibles, health                 │
└────────────────────────────────────────────────────────────────┘
```

### 4.2 Flujo de Voz → Vista

```
1. Usuario toca botón mic en client app
2. Client app graba audio, STT nativo del SO → texto
3. Client app envía texto al client-ia vía P2P request-response
4. Client-ia (Rig agent loop):
   a. LLM analiza query + schema en contexto
   b. LLM decide: ¿existe vista similar? → list_views(tags)
   c. Si match → devuelve ViewDefinition existente (status: "cached")
   d. Si no match → genera SQL → query_entity(sql) para validar
   e. Genera ViewDefinition nuevo → save_view(vd) → CDC sync
   f. Status: "generated"
5. Client-ia responde al client app: { viewDef, status, similarViews }
6. Client app:
   a. Ejecuta SQL localmente (drizzle_execute — datos ya locales)
   b. Renderiza componentes según catálogo
   c. Si status=cached: badge "actualizando..." + refresh background
7. ViewDefinition guardado en DB local de ambos peers (CDC)
```

### 4.3 Cola CDC — Fallback Offline

```
1. Client app intenta P2P request-response (timeout 3s)
2. Si falla → guarda query como entidad P2P: ia_query { id, org_id, text, created_at, status: "pending" }
3. CDC sincroniza al client-ia cuando esté online
4. Client-ia procesa queries pendientes cron (cada 5s)
5. Escribe ia_query.status = "completed" + view_id
6. CDC notifica al client app → badge desaparece, vista aparece
```

### 4.4 Catálogo de Componentes

```typescript
// Componentes que la IA genera y el Shell Universal renderiza

interface DataTableComponent {
  id: string;
  type: "dataTable";
  columns: { key: string; label: string; sortable?: boolean }[];
  onRowClick?: { entity: string };      // navega a entityDetail
}

interface MetricCardComponent {
  id: string;
  type: "metricCard";
  label: string;
  valueKey: string;                     // columna del result set
  format?: "currency" | "number" | "percentage";
}

interface EntityDetailComponent {
  id: string;
  type: "entityDetail";
  entity: string;
  fields: { key: string; label: string; type: string }[];
  relations: { entity: string; label: string; fkField: string }[];
}

interface FormComponent {
  id: string;
  type: "form";
  entity: string;
  action: "create" | "edit";
  fields: { key: string; label: string; type: string; required?: boolean; defaultValue?: unknown }[];
  validations: { field: string; op: string; value: unknown; message: string }[];
  submitLabel: string;
}

interface ColumnComponent {
  id: string;
  type: "column";
  children: string[];                   // component IDs
}

interface RowComponent {
  id: string;
  type: "row";
  children: string[];
}

interface CardComponent {
  id: string;
  type: "card";
  title?: string;
  child: string;                        // single component ID
}
```

### 4.5 ViewDefinition — Estructura

```typescript
interface ViewDefinition {
  id: string;                           // uuid
  orgId: string;
  sql: string;                          // ejecutado localmente por el client app
  entity: string;                       // para permisos
  components: ComponentDef[];           // adjacency list
  root: string;                         // component ID raíz
  meta: {
    naturalLanguageQuery: string;       // "facturas de mayo pendientes"
    createdBy: string;                  // node_id
    createdAt: number;
    lastVisitedAt?: number;
    usageCount: number;
    tags: string[];                     // ["facturas", "mayo", "pendientes"]
  };
}
```

### 4.6 Admin App — Cambios

| Ruta | Cambio |
|---|---|
| `/devices` | Nueva columna `type` (admin/client/client-ia). Acción "Designar como IA de la org" que setea `type: client-ia` |
| `/sync` | Agregar: eventos CDC pendientes por peer, último change_id, latencia estimada, indicador réplica fresca/atrasada |
| `/views` | **NUEVA.** Tabla de ViewDefinitions: nombre (NL query), tags, creador, usos, última visita. Acciones: abrir, archivar, promover a org default |
| `/modules` | **NUEVA.** Templates base disponibles. Módulos activos en la org. Reglas de validación. Instalar/desinstalar. (Mayoría placeholder hasta Fase B) |

### 4.7 Client App — Shell Universal

| Componente actual | Reemplazo |
|---|---|
| Sidebar con `/customers`, `/invoices`, `/products`, `/orders` | Sidebar con: Inicio (vista default), Mis Vistas, Org, Recientes, Inbox |
| `EntityGrid` hardcodeado por ruta | Catálogo de componentes genérico que renderiza cualquier ViewDefinition |
| `DetailPanel` con tabs estáticos | `entityDetail` component generado por IA con fields + relaciones navegables |
| Búsqueda Ctrl+K | Voz (botón mic) + búsqueda texto como fallback |
| Botón "Nuevo" por entidad | Voz: "crear factura para..." → IA genera form |

### 4.8 Benchmark Suite para Modelos

Antes de implementar el client-ia, se corre un benchmark para seleccionar modelos:

| # | Task | Métrica |
|---|---|---|
| 1 | `schema-lookup` | JSON exact match vs schema.json |
| 2 | `sql-generate` | Ejecución exitosa en Limbo de prueba |
| 3 | `view-generate` | Componentes válidos, columnas existen |
| 4 | `form-generate` | Campos requeridos, validaciones |
| 5 | `tool-select` | Tool name + parámetros exactos |
| 6 | `relation-navigate` | JOIN correcto, FK resuelta |
| 7 | `spanish-ambig` | Entidad inferida, filtro correcto |
| 8 | `error-recovery` | Segundo intento exitoso tras SQL inválido |
| 9 | `multi-step` | 2+ queries, componentes compuestos |
| 10 | `module-generate` | Schema + vistas + reglas válidas |

Pipeline: 10 tasks × 5 repeticiones × N modelos = 50N invocaciones. Métricas: precisión, latencia avg/min/max, fallos por categoría. Resultado: matriz de selección ModelRouter.

---

## 5. Fase B — Sistema de Módulos (Visión Documentada)

### 5.1 ModuleDefinition

```typescript
interface ModuleDefinition {
  id: string;
  name: string;
  version: string;
  entities: EntityMeta[];               // schemas de entidades
  views: ViewDefinition[];              // vistas predefinidas
  validationRules: ValidationRule[];    // reglas de negocio
  immutableRules: ValidationRule[];     // reglas contables/fiscales inmutables
  permissions: {                        // permisos por defecto
    defaultRole: { can_open: string[]; can_write: string[] };
  };
  baseTemplate: string;                 // template del que deriva
}
```

### 5.2 Flujo de Módulos

- Admin instala template base → IA lo extiende con campos custom, vistas personalizadas, reglas específicas de la org
- Módulos persisten como entidad P2P (`module_definitions`) vía CDC
- Shell Universal carga el módulo activo y renderiza sus vistas
- Reglas inmutables (chart of accounts, tasas de IVA, partida doble) no pueden ser modificadas por usuarios

### 5.3 Componentes adicionales (Fase B)

- `chart` (bar, line, pie) — requiere recharts/visx
- `report` — tabla con agrupación, subtotales, exportación CSV

### 5.4 Memoria y Embeddings (Fase B)

- Embeddings (`nomic-embed-text` vía Ollama) para matching semántico de vistas
- Preferencias de usuario: `user_preferences` como entidad P2P

---

## 6. Fase C — Hardware + Inter-org + IoT (Visión Documentada)

### 6.1 Fierros Huecos

- Dispositivos commodity (Raspberry Pi 5, mini PC Intel N100)
- NixOS como SO declarativo (consistente con infraestructura existente)
- Auto-update vía Tauri updater (mismo mecanismo que apps desktop)
- `syntrix-client --headless --ia` como systemd service

### 6.2 Tipos de Dispositivo Adicionales

| Tipo | Función |
|---|---|
| `router` | Puente P2P-cloud, relay node, acceso remoto |
| `iot` | Sensor/actuador (temperatura, acceso, energía) con `can_write` limitado |

### 6.3 Comunicación Inter-org

- Canales P2P directos entre peers de distintas orgs
- Factura proveedor (Org A) → factura cliente (Org B) vía libp2p request-response
- Registry P2P de módulos públicos (DHT)

---

## 7. Tareas Ordenadas

### Fase A

1. ✅ **Crear `crates/syntrix-ai/`** — `lib.rs`, `tools.rs`, `router.rs`, `bench.rs`. Tool schemas, `check_read_access` enforcement.
   - `Cargo.toml` con deps: `syntrix-core`, `serde`, `serde_json`, `anyhow`, `tokio`, `tracing`, `uuid`
   - `lib.rs`: `ai_chat_impl` (agent loop scaffold con tool schemas), `ai_status_impl`, `AiContext` trait
   - `tools.rs`: 5 tools (`get_schema`, `query_entity_tool`, `search_entity_tool`, `save_view_tool`, `list_views_tool`) con validación SELECT-only + `check_read_access`
   - `router.rs`: `TaskType` classifier (8 tipos), `ModelRouter` trait, `DefaultRouter` con selección por tarea
   - `bench.rs`: 10 `BenchmarkTask` implementaciones, `run_benchmark_suite`, `compute_summary`
   - Tests: 15 unit tests (tools, router, bench, lib)
   - `crates/syntrix-ai/src/` — 4 archivos, 0 dependencias Tauri (headless-testable)
   - Build: `just test-rust` para compilar remotamente en server-1 (timeout por Nix cache)
   - ModelRouter actualizado con modelos reales del usuario: granite3.2:2b, granite4.1:3b, qwen2.5-coder:3b, deepseek-r1:1.5b

2. ✅ **Agregar Rig + Ollama provider** — `Cargo.toml`: `rig-core`. `OpenAICompatibleProvider` para Ollama (`localhost:11434`). Groq/DeepSeek como cloud fallback.
   - `crates/syntrix-ai/src/provider.rs`: `ModelProvider` trait + `ToolDefinition`, `ChatMessage`, `ToolCall`, `ProviderResponse`, `ProviderChunk`, `MockProvider` para tests
   - `apps/client/src-tauri/src/ai_provider.rs`: `OllamaProvider` implementa `ModelProvider` vía HTTP a Ollama API (OpenAI-compatible format en `localhost:11434/v1`). Soporta `chat`, `chat_stream`, `health`.
   - `apps/client/src-tauri/Cargo.toml`: +`syntrix-ai`, +`rig-core`, +`reqwest`
   - `OllamaProvider` usa `reqwest::blocking::Client` con timeout 120s
   - Test unitario: `test_mock_provider_returns_response`, `test_mock_provider_with_tool_calls`, `test_mock_provider_health`

3. ✅ **Implementar `ai_chat_impl`** — Agent loop real con tool-calling.
   - `ai_chat_impl(ctx, provider, router, req)`:
     1. Clasifica task type → selecciona modelo via router
     2. Build system prompt con schema actual de la DB
     3. Envía mensajes + 5 tool definitions al provider
     4. Si LLM responde con tool calls → ejecuta via `execute_tool()` → añade resultados → repite (max 10 rounds)
     5. Retorna `AiChatResponse` con reply + tool_call_records
   - `execute_tool()` despacha a: `get_schema`, `query_entity`, `search_entity`, `save_view`, `list_views` con `check_read_access` y validación SELECT-only
   - `AiContext` implementado para `AppState` (usa `execute_sql_query` para queries reales contra Limbo)
   - Views almacenadas temporalmente en `OnceLock<Mutex<Vec>>` (hasta Tasks 7-8)

4. **Implementar `ModelRouter`** — Clasifica task type → selecciona modelo. Fallback cascade: local model → larger local model → cloud.

5. **Tauri commands thin wrappers** — `ai_chat(org_id, messages)`, `ai_status()`. Patrón `*_impl(state: &AppState) → Result<T, anyhow::Error>`.

6. **Benchmark suite** — Correr 10 tasks × 5 rep × N modelos contra Limbo de prueba. Generar matriz de precisión/latencia. Seleccionar modelos para ModelRouter.

7. **Entidad `view_definitions` en schema P2P** — Nueva tabla en `schema.json` + migración Drizzle. Columnas: id, org_id, sql, entity, components_json, root, meta_json, created_by, created_at.

8. **Entidad `ia_queries` (cola CDC)** — Tabla para queries pendientes cuando IA offline. Columnas: id, org_id, text, created_at, status, view_id.

9. **Extender Admin `/devices`** — Columna `type` (admin/client/client-ia). Acción "Designar IA". Leer/escribir `type` del device en Limbo.

10. **Extender Admin `/sync`** — Agregar métricas CDC: eventos pendientes, último change_id por peer.

11. **Nueva pantalla Admin `/views`** — Tabla de ViewDefinitions con métricas. Acciones: abrir, archivar, promover.

12. **Nueva pantalla Admin `/modules`** — Placeholder con templates base. Vista completa en Fase B.

13. **Catálogo de componentes en `@syntrix/ui`** — Implementar renderizadores para dataTable, metricCard, entityDetail, form, column, row, card. Cada componente recibe data del result set y se renderiza con shadcn/ui.

14. **Shell Universal en client app** — Reemplazar rutas hardcodeadas. Sidebar: Inicio, Mis Vistas, Org, Recientes, Inbox. Vista activa renderiza ViewDefinition del catálogo.

15. **Voice input en client app** — Botón mic. STT nativo del SO. Texto → `ai_chat` command → ViewDefinition → render.

16. **Cola CDC fallback** — Si `ai_chat` timeout, escribir `ia_query` en DB local. Polling cada 5s para queries completadas.

17. **Migración de entidades actuales** — Las 6 entidades se mantienen como fallback. Si no hay IA configurada, client app muestra navegación tradicional.

### Fase B

18. **`ModuleDefinition` schema** — Nueva entidad P2P `module_definitions`. Columnas: id, name, version, entities_json, views_json, rules_json, immutable_rules_json.

19. **Templates base** — ERP mexicano (chart of accounts, IVA, CFDI fields). Familiar (gastos, despensa, tareas). Comunidad (fondo vecinal, guardias).

20. **IA genera módulos** — NL query → ModuleDefinition con schema, vistas, reglas. Tool `save_module`.

21. **Admin `/modules` completo** — Instalar templates, ver módulos activos, editar reglas de validación.

22. **Componentes chart + report** — `chart` (bar/line/pie con recharts). `report` (tabla con agrupación, export).

23. **Embeddings para view matching** — `nomic-embed-text` vía Ollama. Cosine similarity sobre NL queries históricos.

### Fase C

24. **NixOS config para fierros huecos** — systemd unit para `syntrix-client --headless --ia`. Auto-update. Firewall mínimo.

25. **Tipos `router` + `iot`** — Extender device type enum. Permisos específicos por tipo.

26. **Comunicación inter-org** — libp2p request-response entre peers de distintas orgs. Factura proveedor→cliente.

27. **Registry P2P de módulos** — DHT para descubrir módulos públicos. Instalación cross-org.

---

## 8. Riesgos y Mitigación

| Riesgo | Mitigación |
|---|---|
| LLM genera SQL inválido | Validación pre-ejecución. Retry con error context. Cloud fallback si local falla 2+ veces. |
| Rig API inestable (librería joven) | Thin trait isolation. Swappable a otro provider si es necesario. |
| Modelo local muy lento en hardware barato | granite3.2:2b priorizado (3.6s avg en benchmarks). Cloud fallback automático. |
| CDC sync de vistas congestiona la red | ViewDefinitions son ~2KB JSON. Mínimo overhead vs datos de negocio. |
| IA genera vista con datos de otra org | `check_read_access` en todas las tools. `org_id` vinculado al scope de la sesión. |
| Mutex lock freeze durante llamadas LLM | Lock → extract refs → drop lock → llamar LLM → lock → ejecutar tools → drop. |
| IA offline sin cola CDC | `ia_queries` como entidad P2P. CDC entrega cuando IA vuelve. |
| Sin IA configurada, client app no funciona | Las 6 entidades hardcodeadas son fallback permanente. |
| Hermes Agent parece alternativa a Rig | Hermes es aplicación Python (no librería Rust). No embebible. Rig es la elección correcta. Modelos Hermes sí se usan vía Ollama. |

---

## 9. Validación

1. **Benchmark suite** (`crates/syntrix-ai/src/bench.rs`) — 10 tasks × 5 reps × N modelos. Métricas: precisión, latencia. Corre en CI.

2. **Headless tests** — `ai_chat_impl` con Ollama mockeado: tool calls → query_entity → ViewDefinition válido.

3. **Frontend tests** (vitest) — Catálogo de componentes: render con datos mock. Voice button → STT → `ai_chat` → render.

4. **Integration tests** — Flujo completo: voz → IA genera vista → client renderiza → datos frescos de DB local.

5. **E2E tests** (Playwright) — Client app + client-ia headless. Voice query → vista en pantalla. CDC sync de vistas entre peers.

6. **Regression** — Las 6 entidades actuales siguen funcionales como fallback. `just test-rust` + `just test` deben pasar.

---

## 10. Lo que este plan NO cubre

- Cross-org module sharing o registry (Fase C)
- Comunicación inter-org / transacciones B2B (Fase C)
- WASM runtime (no necesario — sin ejecución de código externo)
- Web client (sin Tauri) — requiere HTTP API desde Rust, esfuerzo separado
- Kanban, calendar, gallery layouts — componentes adicionales Fase B
- Plugin system para terceros — diferente al sistema de módulos IA
- Facturación electrónica SAT (CFDI) — módulo específico, no infraestructura
- Hardware físico específico (modelos, proveedores, precios) — solo specs mínimas documentadas
- Multi-idioma (i18n) — el sistema actual es monolingüe español

---

## 11. Progreso

| # | Tarea | Estado | Fecha |
|---|-------|--------|-------|
| 1 | `crates/syntrix-ai/` — lib, tools, router, bench | ✅ Completo | 2026-07-06 |
| 2 | Rig + Ollama provider (`OllamaProvider`, `ModelProvider` trait) | ✅ Completo | 2026-07-06 |
| | Modelos: granite3.2:2b, granite4.1:3b, qwen2.5-coder:3b, deepseek-r1:1.5b | ✅ Descargados por usuario | 2026-07-06 |
| 3 | `ai_chat_impl` agent loop real (tool-calling, execute_tool, AiContext for AppState) | ✅ Completo | 2026-07-06 |
| 4 | ModelRouter real (Ollama) | ✅ (en `router.rs` con modelos del usuario) | 2026-07-06 |
| 5 | Tauri commands thin wrappers (`ai_chat`, `ai_status`) + headless mode | ✅ Completo | 2026-07-06 |
| 4 | ModelRouter real (Ollama) | 🔲 Pendiente | — |
| 5 | Tauri commands thin wrappers | 🔲 Pendiente | — |
| 6 | Benchmark suite contra modelos reales | 🔲 Pendiente | — |
| 7 | Entidad `view_definitions` | 🔲 Pendiente | — |
| 8 | Entidad `ia_queries` | 🔲 Pendiente | — |
| 9 | Extender Admin `/devices` | 🔲 Pendiente | — |
| 10 | Extender Admin `/sync` | 🔲 Pendiente | — |
| 11 | Admin `/views` pantalla | 🔲 Pendiente | — |
| 12 | Admin `/modules` placeholder | 🔲 Pendiente | — |
| 13 | Catálogo componentes `@syntrix/ui` | 🔲 Pendiente | — |
| 14 | Shell Universal client app | 🔲 Pendiente | — |
| 15 | Voice input | 🔲 Pendiente | — |
| 16 | Cola CDC fallback | 🔲 Pendiente | — |
| 17 | Migración entidades actuales | 🔲 Pendiente | — |

### Docs actualizados tras cada tarea

| Documento | Propósito |
|-----------|-----------|
| `.kilo/plans/1783125232147-syntrix-p2p-ia-platform.md` | Plan maestro + progreso (este archivo) |
| `.ai/AGENTS.md` | Contexto para agentes de IA: nuevo crate, dependencias, patrones |
| `apps/docs/src/content/docs/` | Documentación de Astro Starlight (visión, estado-actual) |
