# Cascada de Inferencia por Capacidad — IA Embebida Local-First

> Reemplaza y absorbe `1783125232147-tool-call-parser.md`. Extiende el plan maestro
> `1783125232147-syntrix-p2p-ia-platform.md` (modifica Decisiones #8, #12; refuerza #1, #11, #16, #18, #19).

## 1. Objetivo

IA generativa embebida **local-first** con una **cascada de inferencia por capacidad del dispositivo**:

```
in-app embebido  →  delegar a peer capaz (P2P)  →  cloud opt-in
```

Salida estructurada **garantizada por gramática** (GBNF) en vez de tool-calling estilo OpenAI,
lo que elimina de raíz el problema del 0% de tool-calling en modelos <4B (ya no hace falta un
parser de texto). Cuando el dispositivo no puede correr una tarea, **se notifica al usuario** y se
delega a un peer capaz o a cloud.

## 2. Contexto de la línea base (verificado en código)

- `crates/syntrix-ai/` ya tiene: `provider.rs` (trait `ModelProvider`, `ToolDefinition`, `ChatMessage`,
  `ToolCall`, `ProviderResponse`, `MockProvider`), `lib.rs` (`ai_chat_impl` agent loop, `execute_tool`,
  `AiContext`), `router.rs` (`TaskType`, `ModelRouter`, `DefaultRouter`), `tools.rs`, `bench.rs`.
- `apps/client/src-tauri/src/ai_provider.rs` = `OllamaProvider` (HTTP a `localhost:11434/v1`,
  `parse_response` solo lee `message["tool_calls"]`, por eso los modelos locales daban 0%).
- **No hay deps de inferencia local** (`llama`/`candle`/`mistral`/`rig-core` ausentes en `Cargo.toml`
  pese a lo que dice el progreso del plan maestro). Partimos limpio.
- Tauri **v2** (móvil viable); `apps/client/src-tauri/gen/` móvil **no** inicializado aún.
- `crates/syntrix-network`: existe `request_response` (behaviour.rs, codecs.rs, lib.rs) y CDC con
  `node_id`/`change_time`/LWW. Reutilizables para delegación y advertising de capacidad.
- Entidad `ia_queries` (cola CDC) ya existe; `device_type` (admin/client/client-ia) ya existe.

## 3. Decisiones de arquitectura (resueltas)

| # | Decisión | Elección | Rationale |
|---|---|---|---|
| A1 | Topología | Cascada por capacidad: in-app → peer → cloud | Cubre móvil potente, fierro hueco y equipo débil con una sola lógica. Máxima soberanía. |
| A2 | Motor primario | **llama.cpp vía `llama-cpp-2`** | Único que cubre móvil (iOS/Android NEON/Metal) + edge (Pi/N100) + desktop, corre GGUF arbitrario (granite/qwen/deepseek de hoy) y trae **GBNF nativo**. |
| A3 | Abstracción | Trait `InferenceBackend` (extiende `ModelProvider`) | Motores swappable por dispositivo. llama.cpp #1; LiteRT-LM (móvil-NPU/web), candle, mistral.rs (desktop-GPU) y cloud como backends alternos documentados. No casarse con un motor. |
| A4 | Salida estructurada | **GBNF single-shot** para `ViewDefinition` | Garantiza JSON válido → elimina tool-calling frágil y el parser de texto. Turnos multi-tool con agent loop se reservan para cloud/7B+. |
| A5 | Parser de texto | Reducido a stripear `<think>...</think>` (deepseek-r1) | GBNF hace el JSON determinístico; el "parser" queda como saneo mínimo previo. Absorbe `tool-call-parser.md`. |
| A6 | Distribución de weights | **P2P-first, mirror fallback** | Peer capaz de la org sirve el GGUF vía libp2p chunked (offline/soberano); si no hay peer → mirror/HF resumable; cache local. |
| A7 | Delegación/discovery | Advertising por CDC + request-response + cola `ia_queries` + cloud | Probe escribe capacidad al registro de device → CDC org-wide → débil elige peer con tier suficiente → request-response; si offline/timeout → cola CDC; último recurso cloud. **Sustituye designación manual admin (#12).** Cero protocolo nuevo. |
| A8 | Tiering | Specs **+ warm-up bench (tok/s medido)**, re-evaluado en power/thermal | Mismo RAM rinde distinto según acceso real al path acelerado (NEON/Metal). Batería/térmico cambian en runtime; no calcular una sola vez al arranque. |
| A9 | Cloud | deepseek v4 flash, **opt-in por org** | Coherente con Decisión #11. Solo para tareas complejas o tras 2 fallos locales. Nunca default. |
| A10 | De-risk | **Spike Nix + NDK/iOS antes de comprometer** | `rustPlatform.bindgenHook` resuelve libclang; el riesgo real está en bindgen+cmake dentro de cross-compile móvil (`pkgsCross.aarch64-android`/`iphone64` vs `cargo-ndk`). |

### Notas sobre motores descartados como primario
- **candle**: sin GBNF nativo (hay que reconstruir sampler con máscara de logits) y solo corre
  arquitecturas porteadas — `granite3.2` (GraniteForCausalLM) **hoy no corre**; Granite 4
  (GraniteMoeHybrid) sí; qwen2.5 reusa arch Qwen2 (OK). Riesgo concentrado en Granite.
- **mistral.rs**: maduro (7.2k★, v0.8.x, `llguidance` = structured outputs más rápido que GBNF,
  soporta Granite 4.0), pero **diseñado para servidor/desktop-GPU** (NCCL, PagedAttention, CUDA
  graphs) — sin iOS/Android. Su capa agentic no aporta aquí porque `syntrix-ai` ya tiene la propia.
  Reevaluar si móvil deja de ser objetivo v1.
- **LiteRT-LM**: mejor NPU/GPU móvil y soporta web (LiteRT.js) e IoT (Pi), pero **sin bindings Rust**
  (C++/Kotlin/Swift) y **no usa GGUF** (formato `.litertlm`, empuja a Gemma). Backend futuro
  detrás del trait para móvil-NPU/web, no fundación.

## 4. Diseño

### 4.1 Trait `InferenceBackend`
- En `crates/syntrix-ai/src/backend.rs`. Extiende/relaciona con `ModelProvider` existente.
- Debe exponer al menos: `chat`/`generate` con **gramática opcional** (GBNF), `chat_stream`,
  `health`, y metadata (`available_models`, `capabilities`).
- Implementaciones: `LlamaCppBackend` (nueva, en `apps/client/src-tauri` o crate propio por el
  toolchain C++), `OllamaBackend` (adaptar el `OllamaProvider` actual como backend de dev/desktop),
  `PeerDelegateBackend` (request-response P2P), `CloudBackend` (deepseek v4 flash). `MockProvider`
  sigue para tests.

### 4.2 Probe de capacidad y tiering
- `crates/syntrix-ai/src/capability.rs`:
  - `DeviceProbe`: RAM total/libre, núcleos/arch, GPU/acelerador, plataforma, storage libre,
    batería/térmico (móvil). Usa `sysinfo` + detección de acelerador por plataforma.
  - **Warm-up benchmark**: cargar el modelo candidato y medir tok/s reales en los primeros segundos.
  - `DeviceTier` = `none | small | medium | large` derivado de specs **+ tok/s medido**.
  - Re-evaluación en eventos de power/thermal (no una sola vez al arranque).
- `ModelManifest`: por modelo `{ arch, quant, min_ram, ctx, size_on_disk, tier }`.
- `TaskTier`: mapea `TaskType` (router.rs) → tier mínimo (voz→vista simple = `small`;
  form/relación = `medium`; módulos/multi-step = `large`/cloud).
- Router extendido: `(DeviceTier × TaskTier) → { Local(model) | Delegate(peer) | Cloud | Unavailable }`.

### 4.3 Notificación de degradación (UX)
- `Unavailable` total → banner: "Este dispositivo no puede correr IA local. Configura un peer IA
  en tu org o activa cloud."
- Capaz de simple pero no de la tarea pedida → prompt: "Esto excede la IA de este dispositivo.
  ¿Uso el peer `<nombre>` / cloud?" con opciones.

### 4.4 Distribución de weights (P2P-first)
- Al decidir `Local(model)` y no tener el GGUF cacheado:
  1. Buscar peer de la org que lo tenga (capacidad advertida vía CDC) → transferencia chunked
     sobre libp2p request-response (resumable, verificación de hash).
  2. Si no hay peer → mirror/registry configurado (HF) resumable.
  3. Cache local en disco; registrar disponibilidad del modelo en el probe/advertising.

### 4.5 Delegación y advertising
- Probe escribe capacidad (tier, modelos disponibles, salud) al registro del device →
  propagado org-wide vía CDC (extiende metadata de device).
- Device débil: consulta DB local por peers con tier suficiente → request-response síncrono
  (timeout ~3s). Si offline/timeout → escribe `ia_queries` (cola CDC async, Decisión #18) →
  peer capaz procesa y responde vía CDC. Último recurso: cloud opt-in.

### 4.6 Grammar GBNF del `ViewDefinition`
- Definir gramática GBNF que restrinja la salida al esquema `ViewDefinition` (sql, entity,
  components adjacency-list, root, meta.tags). Pasarla al backend en el path "voz→vista".
- Saneo previo: stripear `<think>...</think>` antes de parsear (deepseek-r1).

## 5. Tareas ordenadas

1. **Spike de-risk (BLOQUEANTE)** — Proyecto mínimo `llama-cpp-2` que cargue un GGUF chico y genere
   con gramática GBNF, compilando en: (a) desktop Linux vía Nix (`rustPlatform.bindgenHook`),
   (b) cross Android (`pkgsCross.aarch64-android` o `cargo-ndk`), (c) cross iOS (`pkgsCross.iphone64`).
   Documentar el approach de build que funcione. **No avanzar a la arquitectura completa sin esto.**
2. **Trait `InferenceBackend`** — `crates/syntrix-ai/src/backend.rs`, relación con `ModelProvider`,
   soporte de gramática opcional. Adaptar `OllamaProvider` actual como `OllamaBackend`.
3. **`LlamaCppBackend`** — Implementar `InferenceBackend` sobre `llama-cpp-2` con GBNF. Ubicación
   según toolchain (crate propio si el C++ complica el build de `syntrix-ai` puro).
4. **Probe de capacidad + warm-up bench** — `capability.rs`: specs (`sysinfo`) + tok/s medido +
   `DeviceTier`. Re-evaluación en power/thermal.
5. **`ModelManifest` + mapping TaskTier** — manifiesto de requisitos por modelo; `TaskType → tier`.
6. **Router de cascada** — extender `router.rs`: `(DeviceTier × TaskTier) → decisión`.
7. **Grammar GBNF de `ViewDefinition`** + saneo `<think>`. Path voz→vista single-shot.
8. **Advertising de capacidad vía CDC** — escribir tier/modelos/salud al registro de device;
   propagación org-wide; lectura en device débil.
9. **`PeerDelegateBackend`** — request-response P2P para delegar inferencia; fallback a cola
   `ia_queries`; verificación `check_read_access`.
10. **Distribución P2P de weights** — transferencia chunked de GGUF entre peers (hash verify,
    resumable) + fallback mirror/HF + cache local.
11. **`CloudBackend`** (deepseek v4 flash) — opt-in por org; se activa por decisión del router.
12. **UX de notificación de degradación** — banners/prompts en client app según decisión del router.
13. **Deprecar `tool-call-parser.md`** — marcar absorbido; mantener solo el saneo `<think>`.
14. **Cambios al plan maestro** — actualizar Decisiones #8 (motor embebido en vez de Ollama daemon)
    y #12 (advertising por capacidad en vez de designación manual).

## 6. Riesgos y mitigación

| Riesgo | Mitigación |
|---|---|
| Cross-compile móvil de `llama-cpp-2` en Nix | Spike bloqueante (Tarea 1) antes de comprometer arquitectura. |
| Tamaño de transferencia P2P de weights (1-2GB) | Chunked + resumable + hash; preferir peer LAN de la org; cache local. |
| Cambio de backend rompe `granite3.2` | En llama.cpp corre; documentado que candle/LiteRT NO lo corren (usar granite4/qwen si se cambia). |
| Tier mal estimado por specs estáticas | Warm-up bench tok/s + re-evaluación power/thermal. |
| GBNF startup lento | Irrelevante para outputs single-shot chicos; medir en el spike. |
| Backend C++ contamina `syntrix-ai` puro (headless-testable) | Aislar `LlamaCppBackend` en crate/módulo aparte; `syntrix-ai` core sigue sin C++. |

## 7. Validación

1. **Spike** compila y genera con GBNF en Linux + Android + iOS.
2. **GBNF** produce `ViewDefinition` JSON válido en 100% de los casos de prueba (granite/qwen/deepseek).
3. **Cascada** degrada correctamente en 3 escenarios: device fuerte (local), device débil con peer
   (delega), device débil sin peer (cloud opt-in / notifica).
4. **Distribución de weights** transfiere GGUF peer→peer con verificación de hash y reanudación.
5. **Regresión**: las 6 entidades siguen como fallback; `just test-rust` + `just test` pasan.

## 8. Fuera de alcance

- LiteRT-LM / candle / mistral.rs como backends (documentados como enchufables futuros, no v1).
- Web client vía LiteRT.js.
- Agent loop multi-turno con tools en modelos locales (se reserva para cloud/7B+).
- Embeddings para view matching (Fase B del plan maestro).

## 9. Sub-decisiones abiertas (menores, resolver en implementación)

- Firma exacta del trait `InferenceBackend` y cómo compone con `ModelProvider`.
- Formato concreto del `ModelManifest` (TOML/JSON) y su ubicación.
- Gramática GBNF concreta del `ViewDefinition`.
- Config del provider cloud (dónde vive la API key por org, formato).
- Ubicación final de `LlamaCppBackend` (módulo en src-tauri vs crate `syntrix-llama`).
