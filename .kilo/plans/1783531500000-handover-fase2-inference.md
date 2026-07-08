# Handover — Continuación Fase 2: Inferencia embebida local-first

> Rama base: `well-sink`. Este documento secuencia el trabajo de la **próxima sesión** tras cerrar
> el plan de tests. El trabajo principal está detallado en el plan enlazado
> `1783452736636-capability-tiered-inference.md` (14 tareas) — este handover NO lo duplica, solo
> fija prerrequisitos, estado heredado y orden.

## 0. Prerrequisito (gate de arranque)

Antes de tocar Fase 2, completar **`.kilo/plans/1783528011185-finalize-tests-tcp-fix.md`**:
- Fix `.with_tcp()` en `crates/syntrix-network/src/lib.rs` (desbloquea Tier 2/3 + arranque real de apps).
- Tests nuevos: `syntrix-migrate`, `upsert_entity`, `drizzle_execute` typed.
- **Gate**: `just test-rust` verde **incluyendo `sync_test.rs`** (hoy 100% bloqueado) + `just test` (75 frontend).

No arrancar Fase 2 sin este gate: los tests de integración son los que validan la capa DB/P2P sobre
la que se apoya la delegación de inferencia.

## 1. Estado que hereda la próxima sesión

- **Spike `llama-cpp-2`** (Tarea 1 del plan enlazado): estado **PARCIAL**.
  - (a) Desktop Linux vía Nix (`rustPlatform.bindgenHook`): **✓ verificado** — `llama-cpp-2 = "0.1"`
    compila en server-2 (Nix shell con las DEPS del justfile). bindgen+cmake OK en x86_64.
  - (b) Cross Android (`pkgsCross.aarch64-android` / `cargo-ndk`): **PENDIENTE**.
  - (c) Cross iOS (`pkgsCross.iphone64`): **PENDIENTE**.
  - **Sigue siendo BLOQUEANTE**: no avanzar a Tarea 2+ hasta cerrar (b) y (c) o marcarlos fuera de
    alcance de v1 explícitamente (móvil).
- **Reusables ya construidos** (esta sesión y anteriores):
  - `syntrix-relay` binary (`apps/syntrix-relay/`) — relay NAT self-hosted (QUIC + TCP).
  - `crates/syntrix-network`: `request_response` + CDC (`node_id`/`change_time`/LWW) → base para
    delegación P2P y advertising de capacidad. **Nota**: requiere el fix `.with_tcp()` del gate §0.
  - `upsert_entity` + entidad `ia_queries` (cola CDC) → base para la delegación async (Decisión #18).
  - `device_type` (admin/client/client-ia) → base para tiering/advertising.
  - `router.rs` ya tiene `DeviceTier`/`TaskTier`/`CascadeDecision::resolve` (Fase 2 T2-T7 sin backend).

## 2. Trabajo principal (plan enlazado)

Ejecutar **`.kilo/plans/1783452736636-capability-tiered-inference.md`** en su orden de 14 tareas.
Puntos de atención para el ejecutor:

1. **Tarea 1 (spike Android+iOS)** primero — cerrar (b)/(c) del §1. Documentar el approach de build
   que funcione (Nix cross vs `cargo-ndk`). Si móvil se difiere a post-v1, marcarlo explícito y
   proceder con desktop/edge.
2. **Tarea 2 (`InferenceBackend`)** — `crates/syntrix-ai/src/backend.rs`; adaptar `OllamaProvider`
   actual como `OllamaBackend`. Mantener `syntrix-ai` core **sin C++** (headless-testable).
3. **Tarea 3 (`LlamaCppBackend`)** — aislar el C++ en crate propio (ej. `syntrix-llama`) si contamina
   el build de `syntrix-ai` (ver Riesgo del plan enlazado).
4. **Tareas 8-10 (advertising/delegación/weights)** — reusar CDC + request-response + `ia_queries`
   ya existentes; **no** introducir protocolo nuevo.

## 3. Validación / gates

- `just test-rust` + `just test` verdes (incluye regresión de las 6 entidades como fallback).
- GBNF produce `ViewDefinition` JSON válido en 100% de casos (granite/qwen/deepseek).
- Cascada degrada en 3 escenarios: device fuerte → local; débil con peer → delega; débil sin peer →
  cloud opt-in / notifica.
- Transferencia P2P de weights: GGUF peer→peer con verificación de hash + reanudación.

## 4. Fuera de alcance (v1)

- candle / mistral.rs / LiteRT-LM como backends (documentados como enchufables futuros).
- Web client vía LiteRT.js.
- Agent loop multi-turno con tools en modelos locales (reservado para cloud/7B+).
- Embeddings para view matching (Fase B del plan maestro).

## 5. Enlaces

- Plan principal: `.kilo/plans/1783452736636-capability-tiered-inference.md`
- Gate previo: `.kilo/plans/1783528011185-finalize-tests-tcp-fix.md`
- ADR de red (relay/weights): `.kilo/plans/1783470696465-adr-libp2p-vs-iroh-litep2p.md`
- Plan maestro: `.kilo/plans/1783125232147-syntrix-p2p-ia-platform.md`
