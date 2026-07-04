# Syntrix AI Chat + Managed Services + Public Landing

## Goal

Add an AI chat assistant to the Syntrix desktop apps (admin + client) that converses with the user about their ERP data via tool-calling, plus a managed-services backend (Rust/axum, Hetzner) for optional sovereignty-as-a-service features, plus a public multi-language landing page (Astro, Cloudflare Pages) for user acquisition — all with prepaid wallet billing via Stripe.

## Architecture (Final)

```
┌─────────────────────────────────────────────────────┐
│  Tauri Apps (admin/client)                          │
│  ┌──────────────────────┐  ┌──────────────────────┐ │
│  │  Chat Panel (@ui)    │  │  Services Panel (@ui)│ │
│  │  FAB → slide-over    │  │  wallet, usage, keys │ │
│  │  json-render catalog │  │  enable/disable svcs │ │
│  │  streaming (Tauri    │  │  top-up via Stripe   │ │
│  │   event ai_chat_event)│  │  conditional display │ │
│  └──────────┬───────────┘  └──────────┬───────────┘ │
│             │ invoke ai_chat          │ JWT Bearer   │
│  ┌──────────▼─────────────────────────▼───────────┐ │
│  │  Tauri Rust Backend (lib.rs)                   │ │
│  │  ai_chat_impl (Rig agent + tool loop)          │ │
│  │  syntrix-ai-tools (query/search/audit/logs)    │ │
│  └────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
         │ Rig providers              │ JSON/HTTPS
         ▼                            ▼
┌─────────────────┐    ┌─────────────────────────────────┐
│  Groq/DeepSeek  │    │  Services Backend (axum, Hetzner)│
│  Ollama (local) │    │  7 services + billing + gateway  │
└─────────────────┘    └──────────────┬──────────────────┘
                                      │
              ┌───────────────────────┼─────────────────────┐
              ▼                       ▼                      ▼
        Operator Console      Stripe Webhooks        External APIs
        (React/Vite,          (Payment Intents,      (Twilio, Resend,
         @syntrix/ui)          top-up → wallet)       PAC, Hetzner SB)

┌──────────────────────────────────────────────────────┐
│  Public Landing (syntrix.mx)                         │
│  Astro + Tailwind + shadcn/ui — Cloudflare Pages     │
│  Inicio / Características / Precios / Descargas /    │
│  Contacto / Blog — i18n: es, en, pt-BR               │
│  Analytics: CF Web Analytics + Plausible (NixOS)     │
│  Download distribution: R2 + Workers + auto-update   │
└──────────────────────────────────────────────────────┘
```

## Key Decisions (22)

| # | Área | Elección | Rationale |
|---|---|---|---|
| 1 | AI Chat engine | Rig (`rig-core`) | Multi-provider Rust abstraction. Isolated behind own trait. |
| 2 | AI render | `@json-render/react` + Syntrix catalog | Buy engine, build our shadcn catalog. Safe constrained output. |
| 3 | AI tools | `syntrix-ai-tools` crate: read-only over existing APIs | query_entity_advanced, search_entity, audit_query, query_logs, summarize_logs, get_schema |
| 4 | Default provider | Groq `gpt-oss-20b` ($0.075/$0.30 per 1M) | Best price/performance for tool-calling. US-hosted. |
| 5 | Alternative provider | DeepSeek `deepseek-v4-flash` ($0.14/$0.28) | Caching for long sessions. China hosting noted. |
| 6 | Local escape hatch | Ollama (OpenAI-compatible endpoint) | Zero data leaves device. Sovereignty option day one. |
| 7 | Sovereignty tiers | 1) cloud BYOK → 2) Ollama local → 3) Candle in-process | Escalating sovereignty, no lock-in. |
| 8 | PII handling | Ollama local = zero data leaves. Cloud = clear UI warning. | Simple; escape hatch = real sovereignty. |
| 9 | Orchestration | Rust backend (`ai_chat_impl`), headless-testable | Matches convention. Tool execution = local (Redb/Tantivy). |
| 10 | Streaming | Tauri event `ai_chat_event`: `{type: "token"|"tool_call"|"done", ...}` | json-render catalog consumes these shapes. |
| 11 | Backend stack | Rust/axum, PostgreSQL | Coherent with monorepo. |
| 12 | Infrastructure | 1 Hetzner VPS (start → scale) | Simple, predictable cost. |
| 13 | Reverse proxy | Caddy (via NixOS module) | HTTPS automático, HTTP/3 nativo, hot-reload, 10-line Caddyfile. |
| 14 | VPS infra management | NixOS declarativo completo + Colmena | PostgreSQL, Caddy, Plausible, systemd, backups — todo declarativo. |
| 15 | CI/CD | Justfile + `colmena apply` | Extiende remote bridge existente. SSH a Hetzner. |
| 16 | Billing model | Prepaid wallet via Stripe Payment Intents | Prepaid avoids billing friction; common in MX. |
| 17 | Auth (client→backend) | Self-serve JWT: app generates ed25519 keypair → registers → JWT Bearer | Sovereignty-coherent: no manual web sign-up. |
| 18 | Auth (operator→backend) | Credentials + TOTP 2FA, PostgreSQL (argon2), seed CLI | Operator = Syntrix staff, separate audience. |
| 19 | Email transactional | Resend (tenants + Syntrix ops share account) | Modern API, React email, deliverability built-in. |
| 20 | SMS/WhatsApp | Twilio (BSP for both) | Single API for SMS + WhatsApp. WABA directo deferred. |
| 21 | PAC vendor | Deferred (Facturama, Fel, SWIFT — evaluate at time) | Vendor pricing/APIs change. |
| 22 | Key recovery | 3 layers: (1) BIP39 12-word phrase (free), (2) encrypted blob on backend (paid), (3) org admin re-issuance via P2P | Sovereignty-first with convenience upgrade. |
| 23 | CFDI metering | Deduct per timbre via wallet. Idempotency key for webhooks. | Stripe can deliver same webhook multiple times. |
| 24 | App model | App gratuita, managed services pagos | No license management. Revenue from services only. |
| 25 | Landing page | Astro, Cloudflare Pages, multi-page + blog | Separado de apps/docs/ (Starlight dev docs). |
| 26 | Landing pages | Inicio, Características, Precios, Descargas, Contacto, Blog, Blog/[slug] | Content from pitch.md, identidad.md, argumentos-p2p.md. |
| 27 | Landing theme | Tailwind + shadcn/ui | Consistent with @syntrix/ui. |
| 28 | Landing i18n | es + en + pt-BR | MX primary + LATAM + US Hispanics + Brazil. Astro i18n routing. |
| 29 | Landing analytics | CF Web Analytics (base) + Plausible self-host via NixOS | Vitals from CF edge + custom events/funnels from Plausible. |
| 30 | Distribution | Cloudflare R2 + Workers | Worker sirve Tauri updater JSON + download redirects. Zero egress fees. |
| 31 | Auto-update | Tauri updater plugin → Worker JSON endpoint | Signed manifests per platform/target. |
| 32 | Mobile | Diferido a plan separado | Landing shows "coming soon" badges for iOS/Android. |
| 33 | Contact form | POST api.syntrix.mx/api/v1/contact → tabla contact_inquiries + notify via Resend | Reusa infraestructura existente. |
| 34 | Blob backup storage | Hetzner Storage Box (€3.20/mes 1TB, CIFS mount al VPS) | Filesystem directo. Sin proveedor externo. |
| 35 | Monitoreo | Uptime Kuma en server-1 (NixOS), alerta Telegram/email | Open-source, no corre en el VPS que monitorea. |
| 36 | Blog engine | Astro content collections + MDX | Natural fit for Astro. |

## Data Model (Backend)

### PostgreSQL tables (same VPS)

```sql
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id VARCHAR NOT NULL UNIQUE,
    name VARCHAR,
    api_key_hash VARCHAR NOT NULL,
    status VARCHAR DEFAULT 'active',
    wallet_id UUID REFERENCES wallets(id)
);

CREATE TABLE wallets (
    id UUID PRIMARY KEY,
    balance_cents BIGINT NOT NULL DEFAULT 0,
    reserved_cents BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);

CREATE TABLE wallet_transactions (
    id UUID PRIMARY KEY,
    wallet_id UUID REFERENCES wallets(id),
    type VARCHAR NOT NULL,  -- topup, deduction, refund
    amount_cents BIGINT NOT NULL,
    service VARCHAR,
    stripe_payment_intent_id VARCHAR,
    idempotency_key VARCHAR UNIQUE,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE service_rate_cards (
    service VARCHAR NOT NULL,
    unit_label VARCHAR NOT NULL,
    rate_cents_per_unit BIGINT NOT NULL
);

CREATE TABLE service_usage_log (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id),
    service VARCHAR NOT NULL,
    quantity BIGINT NOT NULL,
    cost_cents BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE gateway_api_keys (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id),
    key_hash VARCHAR NOT NULL UNIQUE,
    name VARCHAR,
    rate_limit INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    status VARCHAR DEFAULT 'active'
);

CREATE TABLE key_recovery_blobs (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id),
    device_label VARCHAR NOT NULL,
    encrypted_blob BYTEA NOT NULL,
    encryption_salt VARCHAR NOT NULL,
    created_at TIMESTAMPTZ
);

CREATE TABLE cfdi_stamps (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id),
    pac_reference VARCHAR NOT NULL,
    cfdi_uuid VARCHAR,
    xml_in TEXT,
    xml_out TEXT,
    status VARCHAR DEFAULT 'pending',
    created_at TIMESTAMPTZ
);

CREATE TABLE relay_nodes (
    id UUID PRIMARY KEY,
    tenant_id UUID REFERENCES tenants(id),
    node_id_hex VARCHAR NOT NULL,
    endpoint VARCHAR,
    status VARCHAR DEFAULT 'offline',
    heartbeat_at TIMESTAMPTZ
);

CREATE TABLE operators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR NOT NULL UNIQUE,
    password_hash VARCHAR NOT NULL,          -- argon2
    totp_secret VARCHAR,                     -- TOTP 2FA
    totp_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE contact_inquiries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR,
    email VARCHAR NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

## Concrete Tool Surface (syntrix-ai-tools)

```rust
tools:
  - name: query_entity_advanced
    description: "Query an ERP entity with optional filtering and sorting"
    parameters:
      org_id: string (from system context)
      entity: string (customers, suppliers, products, invoices, orders, payroll)
      filter_field: string?
      filter_value: string?
      sort: object? { field: string, direction: "asc"|"desc" }
      limit: number? (default 50, max 200)

  - name: search_entity
    description: "Full-text search across entities (Tantivy)"
    parameters:
      org_id: string
      query: string
      entities: string[]?
      limit: number? (default 10, max 50)

  - name: get_schema
    description: "Get the full entity schema (fields, types, relations)"
    parameters:
      org_id: string

  - name: audit_query
    description: "Query the audit log for change history"
    parameters:
      org_id: string
      entity: string?
      limit: number? (default 50)

  - name: query_logs
    description: "Query application logs for diagnostics"
    parameters:
      query: LogQuery { target?, span_path?, org?, level?, text?, limit?, offset? }

  - name: summarize_logs
    description: "Get a summary digest of recent logs"
    parameters:
      window_secs: number (default 3600)
```

All tools inherit `org_id` from active org + `check_read_access` at `lib.rs:200`.

## GenUI Catalog (Initial)

```typescript
const syntrixCatalog = {
  components: {
    Table:    { props: { columns, rows, caption? } },           // → shadcn Table
    Metric:   { props: { label, value, change?, changeType? } },// → Card + trend arrow
    Card:     { props: { title, description? }, hasChildren },  // → shadcn Card
    EntityList: { props: { entity, items, onSelect } },         // → interactive list
    BarGraph: { props: { title?, data: [{label, value}] } },    // → future: recharts/visx
  },
  actions: {
    "navigate.to_entity": { params: { entity, id } },
    "app.open_form":      { params: { entity, action, id? } },
  }
};
```

## Infrastructure Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Cloudflare                                              │
│  ├─ Pages: syntrix.mx (landing, Astro build)             │
│  ├─ R2: binarires/ (win/mac/linux + APK)                 │
│  ├─ Workers: /api/update/:target/:arch → updater JSON    │
│  │           /api/download/:platform → 302 to R2         │
│  └─ Web Analytics: vitals, page views                    │
├─────────────────────────────────────────────────────────┤
│  Hetzner VPS (NixOS declarativo, Colmena)                │
│  ├─ Caddy (reverse proxy + HTTPS automático)             │
│  │   ├─ api.syntrix.mx → axum :3001                     │
│  │   └─ operator.syntrix.mx → /var/www/operator/dist    │
│  ├─ axum backend (syntrix-backend)                       │
│  │   ├─ /api/v1/tenants/*                                │
│  │   ├─ /api/v1/wallet/*                                 │
│  │   ├─ /api/v1/gateway/chat/completions                 │
│  │   ├─ /api/v1/cfdi/stamp                               │
│  │   ├─ /api/v1/email/send                               │
│  │   ├─ /api/v1/sms/send                                 │
│  │   ├─ /api/v1/key-recovery/*                           │
│  │   ├─ /api/v1/operator/* (JWT operator scope)          │
│  │   ├─ /api/v1/contact (POST)                           │
│  │   └─ /api/v1/stripe/webhook                           │
│  ├─ PostgreSQL (local, same VPS)                         │
│  └─ Hetzner Storage Box (CIFS mount /mnt/storage-box/)   │
│      └─ blob backups via std::fs                          │
├─────────────────────────────────────────────────────────┤
│  Server-1 (NixOS)                                        │
│  ├─ Remote bridge (bin/cargo): Rust compilation          │
│  └─ Uptime Kuma: monitors api.syntrix.mx + operator      │
└─────────────────────────────────────────────────────────┘
```

## Phased Tasks (Ordered)

### Phase 1: AI Chat in App (through permanent seams)

1. **Create `crates/syntrix-ai/`**
   - `syntrix-ai-tools`: tool definitions + schema descriptions, `check_read_access` integration
   - Tool schemas as Rust types implementing `Serialize` for LLM function-calling JSON

2. **Add Rig + provider support to client/admin `Cargo.toml`**
   - `rig-core` dependency
   - `ModelProvider` trait wrapper (thin isolation layer)
   - `OpenAICompatibleProvider` implementation (configurable base_url, model, api_key)
   - Groq default: `https://api.groq.com/openai/v1`, `gpt-oss-20b`
   - DeepSeek alt: `https://api.deepseek.com/v1`, `deepseek-v4-flash`
   - Ollama: `http://localhost:11434/v1`, user-configured model
   - Config from app settings file (Rust, not webview)

3. **Implement `ai_chat_impl`**
   - System prompt construction: `get_schema_registry` + active org + role + entity descriptions + "read-only, respect access"
   - Rig agent with tool array from `syntrix-ai-tools`
   - Loop: user message → LLM → tool calls → execute locally → results → LLM → final answer
   - Streaming: final answer streamed via `app.emit("ai_chat_event", chunk)`
   - Mutex lock discipline: lock → extract refs → drop lock → call LLM → lock → execute tools → drop → repeat
   - Headless-testable (no Tauri types in impl)

4. **`#[tauri::command] ai_chat` thin wrapper**
   - Takes: `org_id: String, messages: Vec<ChatMessage>, provider_config: Option<ProviderConfig>`
   - Returns: streams events `ai_chat_event`
   - Respects existing `AppState` mutex pattern

5. **Ollama detection**
   - At chat init, try GET `http://localhost:11434/api/tags`
   - If responds → Ollama available. If not → show "Ollama no detectado", default to cloud.

6. **Chat panel UI in `@syntrix/ui`**
   - FAB button component (floating, positioned)
   - Slide-over chat panel with message bubbles
   - json-render provider setup with Syntrix catalog
   - Listens to `ai_chat_event` for streaming tokens
   - Provider config drawer (endpoint, key, model selector)
   - Sovereignty tier selector (cloud BYOK / Ollama local)

7. **json-render Syntrix catalog mapping**
   - Implement initial catalog components (Table, Metric, Card, EntityList) using shadcn/ui primitives
   - Wire streaming render

8. **Integrate into both apps**
   - Add chat panel to `apps/admin/src/App.tsx` and `apps/client/src/App.tsx`
   - Add provider settings to app settings (persisted)

### Phase 2: Managed Services Backend + Infra

9. **Create `apps/backend/` with axum skeleton**
   - `Cargo.toml`: axum, tokio, sqlx (PostgreSQL), jsonwebtoken, stripe-rs, reqwest, serde
   - Database migrations for all tables (data model above)
   - JWT middleware: client Bearer tokens (ed25519 → API key → JWT), operator credentials + TOTP
   - Tenant registration: POST `/api/v1/tenants/register` (ed25519 public key → mints API key → returns JWT)
   - Operator auth: POST `/api/v1/operator/login` (email + password + TOTP)
   - Operator seed: CLI command to create initial operator
   - Health check: GET `/api/v1/health`

10. **NixOS/Colmena config for Hetzner VPS**
    - Colmena hive node definition for `hetzner`
    - NixOS modules: PostgreSQL, Caddy (reverse proxy), syntrix-backend (systemd unit), Plausible
    - Caddyfile: `api.syntrix.mx` → localhost:3001, `operator.syntrix.mx` → static files
    - PostgreSQL: `ensureDatabases = ["syntrix"]`, `backup.enable = true`
    - Build axum binary as Nix derivation from workspace Cargo.toml

11. **Implement Stripe billing**
    - Wallet model + CRUD
    - Stripe Payment Intents endpoint: POST `/api/v1/wallet/topup`
    - Stripe webhook handler: `payment_intent.succeeded` → credit wallet, check idempotency_key UNIQUE
    - Wallet balance: GET `/api/v1/wallet`
    - Transaction history: GET `/api/v1/wallet/transactions`
    - Reserve system: deduct from `reserved_cents` before operation, reject if insufficient

12. **Gateway IA passthrough**
    - OpenAI-compatible chat completions: POST `/api/v1/gateway/chat/completions`
    - Auth via Bearer JWT (client) or X-API-Key (app direct)
    - Proxy to Groq/DeepSeek via Rig
    - Meter usage: count tokens → log to `service_usage_log` → deduct wallet
    - Rate limiting per API key (leaky bucket, 100 req/min default)
    - Return standard OpenAI streaming response

13. **CFDI PAC integration**
    - PAC abstraction trait: `stamp(xml: &str) -> Result<StampedCfdi>`
    - Mock implementation for testing (returns dummy timbre)
    - CFDI stamping: POST `/api/v1/cfdi/stamp` → relay to PAC → return stamped XML
    - CFDI status: GET `/api/v1/cfdi/{uuid}/status`
    - Metering: deduct per timbre
    - CFDI stamping log table

14. **Contact form endpoint**
    - POST `/api/v1/contact` (name, email, message)
    - Store in `contact_inquiries` table
    - Notify staff via Resend email

15. **Resend email integration**
    - Email sending module using Resend API
    - Tenant transactional emails: POST `/api/v1/email/send` (metered, per-tenant billing)
    - Operational emails: contact notifications, operator password reset, alerts (not billed)

16. **Operator console (new React/Vite web app)**
    - Create `apps/operator/` with Vite + React + `@syntrix/ui`
    - Operator auth: login page, TOTP 2FA
    - Tenant list: search, status, wallet balance, usage summary
    - Tenant detail: wallet transactions, service usage, API keys, key recovery blobs
    - Rate card management: view/edit per-service rates
    - Revenue dashboard: top-ups, deductions, active tenants
    - CFDI stamping log: history view

17. **Client services panel in Tauri apps**
    - Register with backend: generate ed25519 keypair → POST `/api/v1/tenants/register` → store API key
    - Wallet display: balance, low-balance warning, top-up button (opens Stripe Checkout)
    - Usage display: per-service usage + cost
    - API keys display: gateway key management
    - Toggle services: enable/disable per service
    - Conditional rendering: shown only when connected to backend (has API key stored)

### Phase 3: Public Landing Page + Distribution

18. **Create `apps/landing/`**
    - Astro + Tailwind + shadcn/ui
    - i18n routing: `/[lang]/...` (es, en, pt-BR)
    - Pages: index.astro, features.astro, pricing.astro, download.astro, contact.astro
    - Blog: content collections + MDX, `[lang]/blog/[...slug].astro`
    - Contact form: POST to `api.syntrix.mx/api/v1/contact`
    - Download page: platform-detect + direct links to R2 Workers
    - Deploy: Cloudflare Pages via `wrangler pages deploy`

19. **Cloudflare R2 + Workers for distribution**
    - R2 bucket: `syntrix-releases` for binaries (win/mac/linux + APK)
    - Worker: `/api/update/:target/:arch` → Tauri updater JSON (signed manifests)
    - Worker: `/api/download/:platform` → 302 redirect to R2 object
    - Worker: download analytics logging
    - CI: build → upload to R2 (primary) + GitHub Releases (mirror)

20. **Tauri auto-update config**
    - `tauri.conf.json`: updater endpoint → `https://syntrix.mx/api/update/{{target}}/{{arch}}`
    - Private key for signing update manifests
    - CI generates update JSON with signatures per platform

21. **Blog initial content**
    - Migrate/adapt pitch.md, identidad.md, argumentos-p2p.md as blog posts
    - Post schedule: product announcement, technical deep-dive, P2P philosophy
    - MDX with Astro content collections

22. **Plausible self-host setup**
    - NixOS module for Plausible
    - Caddy virtual host: `analytics.syntrix.mx`
    - Custom events: Download click, Contact submit, Pricing page visit
    - Funnel: Landing → Features → Pricing → Download → Download click

### Phase 4: Remaining Managed Services

23. **SMS/WhatsApp (Twilio)**
    - Twilio client integration, metering per message
    - POST `/api/v1/sms/send` and `/api/v1/whatsapp/send`

24. **Always-on relay node**
    - Iroh relay endpoint monitoring, heartbeat tracking
    - Metering per month

25. **Key recovery**
    - Encrypted blob upload/download endpoints
    - Passphrase-enforced encryption (backend NEVER sees key)
    - POST `/api/v1/key-recovery/store`, GET `/api/v1/key-recovery/retrieve`

26. **Backup de blobs**
    - Iroh blobs mirror to Hetzner Storage Box (CIFS mount)
    - `std::fs::write("/mnt/storage-box/{tenant_id}/{blob_hash}", data)`
    - Metering per GB/month

### Phase 5: Uptime Monitoring

27. **Uptime Kuma on server-1**
    - NixOS module for Uptime Kuma
    - Monitors: `api.syntrix.mx`, `operator.syntrix.mx` (HTTP 200 check)
    - Alerts: Telegram bot + email to operators
    - Check interval: 60 seconds

### Phase 6: Sovereignty Premium Tier (future)

28. **Candle in-process provider** — local LLM within Tauri binary, no daemon
29. **Self-host GPU inference** — Hetzner GPU / Akash for gateway sovereignty tier
30. **MCP server** — stdio/HTTP adapter over syntrix-ai-tools

## Convention Requirements

- Every `#[tauri::command]` wraps a `*_impl(state: &AppState | &mut AppState, ...) -> Result<T, anyhow::Error>` with NO Tauri types.
- Headless tests via `just test-rust` (remote bridge on server-1).
- AI-first logging: `syntrix_span!` for canonical operations (add `ai_chat`, `ai_tool_call`).
- Zero secrets in logs: keypairs, API keys, invite tickets auto-redacted.
- Shared UI in `@syntrix/ui/` for parity between admin and client apps.
- NixOS declarative configs for all infrastructure.

## Risks & Mitigation

| Risk | Mitigation |
|---|---|
| Rig API churn (young library) | Thin trait isolation — swap bounded to `ModelProvider` trait |
| DeepSeek hosting China + MX fiscal data | Groq default, DeepSeek optional, Ollama escape hatch |
| Mutex lock freeze during LLM calls | Lock → extract → drop → LLM → lock → execute → drop pattern |
| PAC downtime / rate limits | Retry queue, idempotency, fallback to second PAC |
| Stripe webhook delivery failures | Idempotency keys, periodic reconciliation job, alerting |
| Wallet going negative | Reserve system: deduct from `reserved_cents`, reject if insufficient |
| Hetzner single point of failure | Acceptable for MVP — services are opt-in, not core P2P sync |
| Cloud AI sends business data off-device | Ollama local = sovereignty escape hatch. Cloud users see warning. |
| User loses key recovery passphrase | BIP39 phrase = primary. Encrypted backup = convenience. Admin re-issuance = fallback. |
| Storage Box single datacenter | Acceptable for encrypted blob backups. Not core sync data. Mitigate at scale. |

## Validation Plan

1. **Headless tests** (`just test-rust` on server-1):
   - `ai_chat_impl` with mocked/ollama provider: tool calls → query_entity → results
   - `syntrix-ai-tools`: schema generation, check_read_access integration
   - Backend axum: JWT auth, wallet top-up/deduction, gateway passthrough, CFDI relay

2. **Frontend tests** (vitest):
   - Chat panel: message rendering, json-render catalog integration, streaming
   - Services panel: balance display, top-up flow, key management
   - Operator console: tenant CRUD, rate card management

3. **Integration tests**:
   - Stripe test mode: top-up → webhook → wallet credit → CFDI stamp → deduction
   - Gateway IA: API key → chat completions → metering → wallet deduction
   - End-to-end: AI chat in app → tool call → query_entity → json-render card

4. **Landing validation**:
   - Lighthouse score > 90 (static Astro)
   - i18n routing works for all 3 languages
   - Download flow: click button → Worker redirect → binary downloaded
   - Contact form: submit → stored in DB → email received
   - Plausible: custom events fire for Download, Contact, Pricing

## Open Questions

- **PAC vendor**: Facturama, Fel (Finkok), SWIFT — evaluate pricing/API at implementation time.
- **Rate card defaults**: per-service pricing to be defined when billing module is implemented (business decision).
- **Operator console exact screens**: refine during implementation.
- **PAC mock implementation**: define `PacProvider` trait with mock that returns dummy timbre for testing.
- **Plausible NixOS module**: verify existence in nixpkgs; if not, use Docker container via NixOS.
