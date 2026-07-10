# Plan: "Company OS" AI-first (solo-fundador) con Paperclip como espinazo

> **Estado**: Aprobado para implementación.
> **Fecha**: 2026-07-08 (rev. 2026-07-09: re-validado vs. Mastra/Rig/OpenHands — decisión sin cambios; ver §1).
> **Ámbito**: infraestructura (`infra-core`: NixOS/Colmena) — **no** `syntrix-monorepo`.
> **Objetivo**: montar un control plane para operar un portafolio de proyectos con un equipo de agentes de IA, gestionado como empresa (org chart, budgets, gobernanza), accesible desde el móvil.
> **⚠️ Implementación**: requiere edición de config + comandos mutantes → **cambiar a un agente con permisos de implementación**. Este plan no ejecuta cambios.

---

## 1. Decisión de arquitectura

- **Espinazo = Paperclip** (`paperclipai/paperclip`, MIT, TypeScript, Node+Postgres). Control plane "empresa de agentes": companies (multi-tenant), org chart, tickets, heartbeats/routines, **budgets con hard-stops**, gobernanza/aprobaciones, portabilidad.
- **Forma:** dashboard web (no chat). Agentes = "empleados" **BYO** (Claude Code, Codex, Cursor, aider/CLI, OpenClaw vía HTTP, y el propio `syntrix-ai` vía HTTP/heartbeat).
- **Descartado como espinazo (con razón):** AgentTeams (chat/Matrix, stack pesado podman+Higress+MinIO, 5k★, Alibaba-céntrico) y AgentScope (framework Python para *construir* agentes). Ver §7 (alternativas/fallback).
- **Producto Syntrix intacto:** `crates/syntrix-ai` sigue soberano/offline; Paperclip lo *opera/construye*, no se embebe en él.

### Por qué Paperclip a futuro (resumen del análisis)
Multi-company nativo (portafolio), budgets/hard-stops (control de costos con DeepSeek/24-7), dashboard que escala mejor que chat, BYO-agent (compone con lo que ya usas), TS+Postgres (tu stack, mantenible), footprint ligero, `--bind tailnet` (calza con Headscale). AgentTeams solo ganaría si el UX principal fuera chatear por Matrix o querer workers contenedores turnkey.

### Veredicto vs. alternativas evaluadas (rev. 2026-07-09)

Se re-evaluó el espinazo contra frameworks recientes (Mastra, Rig, LangGraph, AutoGen, OpenHands). Conclusión: **no cambia** — lo refuerzan. Clave: distinguir **dos capas**:

- **Control plane (capa de negocio)** — gestiona la *empresa* de agentes: portafolio multi-company, budgets/hard-stops, org chart, gobernanza, tickets, routines, dashboard móvil. **Aquí compite Paperclip.**
- **Framework de ejecución/autoría (capa de agente)** — *construye/corre* un agente concreto: tools, control de flujo, memoria. **Aquí viven Rig, Mastra, LangGraph, AutoGen** — detrás de la frontera BYO (§5), **no** compiten con Paperclip.

**Paperclip vs OpenHands** (único rival de control plane serio, tras su viraje a "developer control center"):

| Requisito del ADR | Paperclip | OpenHands (Agent Canvas) |
|---|---|---|
| Portafolio multi-company aislado (§4) | ✅ nativo | ❌ multi-backend, no multi-company |
| Budgets con hard-stop por agente/company (§9) | ✅ nativo (pausa + cancela) | ❌ solo límite por tarea |
| Org chart / gobernanza / audit (§4) | ✅ nativo | ⚠️ ligero, engineering-centric |
| Footprint en server-2 (15 GiB, sin pelear con builds) | ✅ 1 proceso Node + Postgres embebido | ❌ Python+Node+Docker (pesado) |
| Stack afín (§1) / dashboard no-chat | ✅ TS+Postgres, task-manager | ❌ políglota; gira en torno a conversaciones |

**Regla de decisión:** administrar una *empresa* de agentes → **Paperclip** (tu caso). IDE-agente self-hosted para picar código → OpenHands (no es tu caso; descartado explícitamente el UX chat en §1).

**Capa de ejecución para Syntrix:** `crates/syntrix-ai` hoy trae un router/loop de agente hecho a mano (`reqwest`); **Rig** (Rust, MIT, providers DeepSeek/Ollama/local, compila a WASM) es el candidato natural para reemplazarlo. Es del *producto*, detrás de BYO — **no toca el espinazo**.

## 2. Host y despliegue

- **Host = `server-2`** (verificado: 6 núcleos, 15 GiB RAM, 166 GB libres, idle, **Node v24.15 ya instalado**, always-on, NixOS/Colmena).
  - Descartados: `laptop-rao` (no always-on, daily-driver limitado), `laptop-acer-azul` (8GB; queda libre como worker extra o para portales nativos futuros), `server-1` (saturado/flaky, disco lleno).
- **Modo:** servicio **NixOS/systemd** reproducible (no `pnpm dev` suelto). Postgres embebido para arranque; migrar a Postgres propio si escala.
- **Acceso:** `--bind tailnet` sobre la malla **Headscale/Tailscale** existente. **Nunca público.** Desde el móvil vía cliente de la malla.
- **Telemetría:** desactivar (`PAPERCLIP_TELEMETRY_DISABLED=1` o `DO_NOT_TRACK=1`).

## 3. Frontera de coding (Syntrix) — regla dura

- El "empleado" de coding trabaja en **workspaces / git worktrees** de Paperclip, pero **NO compila Rust local**.
- **Builds de Syntrix pineados a `REMOTE_HOST=server-1`** vía el `bin/cargo` bridge (failover automático a server-2 si server-1 cae). Motivo: evitar que un build pesado contienda con Paperclip, que corre en server-2.
- Gates de Syntrix intactos: `just test-rust` / `just lint` como criterio de "hecho".

## 4. Modelo de datos del "Company OS"

- **Company = proyecto/tenant** (aislamiento total). Primer company: **Syntrix**. Luego: los del `plan/` (ai, CDN, git, windmill, first).
- **Empleado = agente** con rol, título, reporting line, **presupuesto mensual (hard-stop)** y skills.
- **Routines = cron/webhook** para trabajo recurrente (reportes, mantenimiento, sync).

## 5. Hedge de intercambiabilidad (obligatorio)

- Mantener el control plane **swappable**: agentes y repos **BYO desacoplados** (no acoplar `syntrix-ai`, Claude Code, aider ni los repos a Paperclip).
- Usar **export/import de companies** de Paperclip para portabilidad. Si en 12 meses aparece algo mejor, se migra sin rehacer los agentes.

## 6. Tareas ordenadas (para el agente de implementación)

1. **Onboarding de red/acceso**
   - Alias SSH `Host laptop-acer-azul` → `192.168.1.71` (si se usa como worker extra).
   - Confirmar que `server-2` está en la malla Headscale/Tailscale.
2. **Servicio Paperclip en `server-2` (NixOS)**
   - Añadir módulo/servicio NixOS para Paperclip (Node 24 ya presente): `npx paperclipai onboard --yes --bind tailnet` como referencia del comportamiento; empaquetar como systemd unit reproducible.
   - Postgres embebido inicial; dir de datos/almacenamiento en disco con headroom.
   - `PAPERCLIP_TELEMETRY_DISABLED=1`. Puerto API (3100) solo por malla/loopback.
3. **Validar arranque** → UI accesible por tailnet desde el móvil; crear la primera board user.
4. **Company "Syntrix"** + goal inicial; registrar el repo/proyecto.
5. **Contratar empleados (BYO)**: 1 coding (Claude Code/Codex/aider) + 1 ops (OpenClaw vía HTTP) — con **budget por agente** y **routines** mínimas.
   - Empleado de coding de Syntrix: workspace con worktree; **`REMOTE_HOST=server-1`** para `cargo`.
6. **Gobernanza**: activar approvals/audit log; definir hard-stops de presupuesto.
7. **Backups** del Postgres + almacenamiento de Paperclip (integrar con restic existente).
8. **(Después)** onboard un 2º proyecto como company para validar herencia/aislamiento; memoria/knowledge; más "departamentos".

## 7. Alternativas documentadas (no adoptar ahora)

- **AgentTeams (HiClaw)** — si más adelante quieres UX de **chat/Matrix** + workers contenedores turnkey. Ya existe `nixos/modules/ai/hiclaw` (v1.0.9, atrasado, registry China; desplegable en server-2/Acer). Fallback "chat".
- **AgentScope** — framework Python ligero (Agent Service + Web UI + multi-tenant) si prefieres **construir** agentes en código. Fallback "DIY ligero".
- **OpenHands (Agent Canvas)** — control center self-hosted para coding agents + automations (Python+Node+Docker, ~80k★). Solapa parcialmente con Paperclip pero **sin** multi-company / budgets-hard-stop / org-chart. Uso futuro: como **runtime de ejecución BYO bajo Paperclip** (enganchar solo su **Agent Server** REST, no el Canvas, para no duplicar dashboards). **No es adapter nativo de Paperclip** → requiere escribir un adapter HTTP/webhook o plugin (`adapter-plugin.md` / `tools/agent-shim`). Adoptar solo si la soberanía del runtime de coding se vuelve requisito duro.
- **Frameworks de ejecución (para *construir* empleados a medida, detrás de BYO §5)** — **Rig** (Rust, MIT, afín a `syntrix-ai`), **Mastra** (TS, Apache-2.0 + módulos `ee/` source-available), **LangGraph/AutoGen** (Python). Principio: la **durabilidad/estado (suspend/resume, human-in-the-loop persistido) vive en el agente, no en el control plane**. Adoptar solo cuando una tarea exija flujo determinista/durable; para coding, un agente off-the-shelf (Claude Code/aider) suele rendir más que construir uno.
- **Portales nativos** (OpenClaw Canvas/Control UI, Hermes TUI) — solo corriendo esos agentes **standalone** (host con más RAM, p.ej. la Acer). Diferidos.

## 8. Validación

- Paperclip arranca como servicio en server-2 y su UI es accesible **solo por tailnet** desde desktop y móvil.
- Se crea company Syntrix, se contrata un empleado con budget, y una tarea se ejecuta de punta a punta con **audit log + costo registrado**.
- El empleado de coding de Syntrix corre `just test-rust` (build en **server-1**) sin compilar Rust en server-2.
- Un hard-stop de presupuesto **pausa** al agente al alcanzar el límite.
- Con Paperclip activo + un build de Syntrix en curso, la UI sigue respondiendo (contención tolerable).

## 9. Riesgos y mitigación

| Riesgo | Mitigación |
|---|---|
| Contención Paperclip↔build en server-2 | Builds pineados a server-1 (+failover); builds bursty, Paperclip idle liviano |
| Costos de tokens desbocados (24-7) | Budgets con **hard-stops** por agente/company (feature nativa) |
| Lock-in al control plane | BYO-agent desacoplado + export/import de companies (§5) |
| Paperclip open-core / cloud upsell futuro | MIT hoy y **maduro** (73k★, v2026.707.0; budgets/gobernanza/routines/multi-company ya entregados). Hedge de portabilidad (§5); alternativas §7 |
| Exposición accidental | Solo tailnet, nunca público; telemetry off |
| server-1 flaky/disco lleno para builds | Bridge con failover a server-2; vigilar disco (nix GC) |

## 10. Fuera de alcance

- Embeber cualquier orquestador en el producto Syntrix.
- Correr el control plane en laptop-rao (no always-on).
- Portales nativos (Canvas/TUI) en esta fase.
- Migración a Postgres externo / despliegue cloud (futuro, si escala).
- Implementación en sí (requiere agente con permisos de escritura + comandos mutantes).

## 11. Decisiones abiertas menores (resolver en implementación)

- Empleado coding inicial: **Claude Code o aider** (adapters *first-class* de Paperclip → cero integración; elegir por costo/DeepSeek). **OpenHands descartado para el MVP** (no es adapter nativo → costo de integración + footprint pesado; ver §7).
- Empaquetado NixOS: módulo custom vs contenedor; según reproducibilidad deseada.
- Ubicación de este plan: se mantiene en `.kilo/plans/` de syntrix; considerar copiarlo a `infra-core` al implementar.
