# ADR: Frameworks de agentes externos (trending) vs. arquitectura Syntrix + plan de adopción de dev-tooling

> **Estado**: Aceptado.
> **Fecha**: 2026-07-08.
> **Ámbito**: producto (`crates/syntrix-ai`) y flujo de trabajo de agentes de IA del monorepo (`.kilo/`, `.ai/AGENTS.md`).
> **Origen**: evaluación de una lista de repos "trending" (21 proyectos) para decidir si conviene integrarlos como app/servicio del producto o como tooling de desarrollo.
> **Método**: auditoría directa de los repos (READMEs, lenguaje, arquitectura). Se marca cuáles fueron **[auditados]** a fondo y cuáles **[clasificados]** por categoría.

---

## 1. Contexto — restricciones que fijan la decisión

Del plan maestro (`.kilo/plans/1783125232147-syntrix-p2p-ia-platform.md`) y `.ai/AGENTS.md`:

- **Soberanía / offline-first / P2P** (libp2p + CDC sobre Limbo). Sin servidores, sin VPN.
- **IA embebida como librería Rust** en el binario `syntrix-client` — *no* un servicio/daemon aparte (decisión #16).
- **Sin ejecución de código externo** — WASM rechazado (decisión #15); tools read-only + `save_view` (decisión #19).
- Ya existe **`crates/syntrix-ai`**: agent loop con tool-calling, `ModelRouter`, cascada `Local/Delegate/Cloud`, providers Ollama/Mock, benchmark.
- El plan maestro **ya rechazó `hermes-agent`** por no embebible (riesgo #491) y **ya cita A2UI** como inspiración de su catálogo de UI (decisión #2).

**Topología de máquinas** (verificada en `bin/cargo`):
- El **harness del agente corre en la laptop** (`~/.claude`, `~/.config/kilo/`; contexto versionado en `.ai/AGENTS.md` y `.kilo/plans/`).
- **server-1 / server-2** son **ejecutores de compilación headless** vía SSH+rsync; el agente nunca corre ahí y el árbol se sobrescribe con `rsync --delete` en cada build.

## 2. Decisión

1. **No integrar ninguno de los 21 repos como app extra ni servicio del producto.** `syntrix-ai` sigue siendo el camino correcto.
2. **Sí adoptar conceptos y skills de dev-tooling** encima del flujo actual `.kilo/` + `.ai/AGENTS.md`, **repo-scoped**, **solo en la laptop**. Nada en server-1/2.
3. Los conceptos de **UI/diseño** y **memoria** se registran como insumos para la **Fase B** de `syntrix-ai` (catálogo/tokens y embeddings/memoria), no como dependencias externas.

## 3. Eje Producto — por qué nada se embebe

Criterios: embebible en Rust · offline-first · P2P/CDC · sin código externo · tools read-only · soberanía.

- **openclaw** [auditado]: daemon Node multi-canal (WhatsApp/Telegram/…), Live Canvas (A2UI), providers cloud. Duplica `syntrix-ai`, no local-first ni embebible → ❌. *Concepto A2UI ya en el plan (decisión #2).*
- **hermes-agent** [auditado]: agente Python auto-mejorable (memoria, skills, gateway). Ya rechazado; sus *modelos* sí vía Ollama → ❌ como código.
- **Resto**: son tooling de coding o off-topic; no son features de un ERP P2P → ❌.

**Conclusión producto:** mantener `syntrix-ai`. Los únicos aportes son *conceptos* (A2UI, memoria, inteligencia de diseño) para Fase A/B.

## 4. Eje Dev-tooling — shortlist útil (qué es · cómo usar · cómo integrar)

Instalación de todo lo siguiente: **laptop, repo-scoped en `.kilo/`** (versionado, compartido entre worktrees). El `rsync` al servidor incluirá estos markdown (inocuo).

### 4.1 superpowers — metodología SDLC como skills [auditado]
- **Qué es**: framework de skills (`SKILL.md`) para agentes de coding: brainstorming, writing-plans, executing-plans, subagent-driven-development, test-driven-development, requesting-code-review, using-git-worktrees, finishing-a-development-branch. Soporta OpenCode y AGENTS.md nativamente. (250k★, MIT)
- **Cómo usar**: sus skills se disparan automáticamente por tarea; formalizan lo que hoy haces a mano (`.kilo/plans/`, worktrees del Agent Manager, gates de aprobación).
- **Cómo integrar**: portar los `SKILL.md` de mayor valor (`writing-plans`, `test-driven-development`, `requesting-code-review`, `using-git-worktrees`) a `.kilo/skill/`, adaptándolos a las reglas Syntrix (compilación remota vía `bin/cargo`, gate `just test-rust` verde, convención `*_impl` headless). Referenciarlos desde `.ai/AGENTS.md`.

### 4.2 anthropics/skills — spec + template canónico [auditado]
- **Qué es**: repo oficial de Anthropic con la **spec de Agent Skills** + template `SKILL.md` (frontmatter `name` + `description`). (159k★)
- **Cómo usar**: como referencia de formato, no como pack a instalar.
- **Cómo integrar**: basar cada `.kilo/skill/*/SKILL.md` en el template + spec para mantenerte estándar y portable entre harnesses.

### 4.3 ECC — instincts + memoria + seguridad [auditado]
- **Qué es**: "harness OS" multi-harness (Claude Code, Codex, Cursor, OpenCode…): 261+ skills, *instincts* (aprendizaje continuo con confidence), memoria vía hooks, AgentShield (security scan). Tier Pro de pago. (227k★, MIT)
- **Cómo usar**: selectivamente; NO instalar el harness completo (pesado/opinado).
- **Cómo integrar**: (a) adoptar el patrón *instinct* como una sección curada "Instincts / Lecciones aprendidas" en `.ai/AGENTS.md`; (b) tomar el concepto AgentShield para revisar comandos que ejecuta el agente antes de aprobarlos.

### 4.4 UI/diseño — ui-ux-pro-max-skill + awesome-design-md [auditados]
- **Qué es**: `ui-ux-pro-max-skill` (103k★): SKILL con 67 estilos, 161 reglas por industria, paletas, tipografía, stacks incl. shadcn/React; target `--ai kilocode`; tier premium. `awesome-design-md` (98k★): colección de `DESIGN.md` (concepto Google Stitch) que los agentes leen para generar UI consistente.
- **Cómo usar**: doble uso — dev-tooling (generar UI shadcn consistente) y producto (nutrir el catálogo de componentes/tokens).
- **Cómo integrar**: (a) **dev-tooling**: crear un `DESIGN.md` en la raíz del repo con el lenguaje visual de Syntrix (colores, tipografía, componentes shadcn, do/don't); markdown puro, cero lock-in. (b) **producto (Fase B)**: extraer estilos/tokens/anti-patrones al catálogo de `packages/syntrix-ui` y al `ViewRenderer`/`ViewDefinition`. Probar el skill de UI repo-scoped y decidir keep/drop tras un trial.

### 4.5 memoria de agente — claude-mem / hermes / ECC [claude-mem auditado]
- **Qué es**: `claude-mem` (86k★, Apache): memoria persistente entre sesiones con hooks + worker HTTP local + **SQLite/FTS5 + Chroma** + búsqueda en 3 capas. Concepto compartido con hermes y ECC.
- **Cómo usar**: solo como **referencia conceptual** ahora.
- **Cómo integrar**: informa la Fase B de `syntrix-ai` (embeddings/memoria); su stack SQLite/FTS5 ya coincide con el tuyo (Limbo/Tantivy). ⚠️ Adopción diferida y con cautela: `claude-mem` incluye token cripto (CMEM) y corre un servicio worker — preferir concepto sobre dependencia.

## 5. Descartados (y por qué)

- **Harnesses/plataformas** — `anthropics/claude-code`, `anomalyco/opencode`, `earendil-works/pi`, `code-yeongyu/oh-my-openagent`: no se "integran en Syntrix"; son la plataforma sobre la que corre el agente. Ya usas **Kilo**. Nota: superpowers/ECC/ui-skill soportan OpenCode si algún día migras de harness.
- **Packs de skills/metodología adicionales** [clasificados] — `andrej-karpathy-skills`, `msitarzewski/agency-agents`, `mattpocock/skills`, `garrytan/gstack`, `gsd-build/get-shit-done`, `shanraisshan/claude-code-best-practice`, `shareAI-lab/learn-claude-code`: redundantes con superpowers/ECC; canibalizar ideas puntuales si hacen falta, sin instalarlos.
- **Utilidad marginal** — `farion1231/cc-switch` [auditado]: gestor de providers multi-harness; útil solo si haces malabares entre harnesses; ⚠️ README saturado de anuncios de relays y **no gestiona Kilo**.
- **Novelty** — `ultraworkers/claw-code` [auditado]: el propio README dice que es "museum exhibit", no producto; `cargo install claw-code` instala un stub deprecado.
- **Off-topic (ni producto ni tooling)** — `ruvnet/RuView` (sensing WiFi), `666ghj/MiroFish` (swarm/predicción), `karpathy/autoresearch` (auto-entrenar nanochat), `D4Vinci/Scrapling` (web scraping; Syntrix es offline/P2P, sin scraping).

## 6. Instalación (dónde)

- **Solo laptop**, **repo-scoped en `.kilo/`** (y `DESIGN.md` en la raíz). Versionado y compartido entre worktrees.
- **Nada en server-1/server-2** (ejecutores de compilación; se borran con `rsync --delete`).
- Global en `~/.claude` / `~/.config/kilo` solo para preferencias personales, no para tooling del proyecto.

## 7. Tareas ordenadas (adopción)

> Requiere un agente con permisos de escritura de código/config. Este ADR no las ejecuta.

1. **`DESIGN.md` en la raíz del repo** — documentar el lenguaje visual Syntrix (paleta, tipografía, componentes shadcn, spacing, do/don't). Seed a partir de patrones de `awesome-design-md` y de `packages/syntrix-ui`.
2. **Crear `.kilo/skill/`** con 3–4 skills portados de superpowers, escritos según la spec de `anthropics/skills`:
   - `writing-plans` (alineado a `.kilo/plans/` + `plan_exit`).
   - `test-driven-development` (con gate `just test-rust`/`just test`, convención `*_impl` headless).
   - `requesting-code-review` (checklist previo, severidades).
   - `using-git-worktrees` (alineado al Agent Manager / `.kilo/worktrees/`).
3. **Sección "Instincts / Lecciones aprendidas" en `.ai/AGENTS.md`** (patrón ECC) — semillas: no correr `cargo` local (usar `bin/cargo`), no editar `.sql` de migraciones, reglas CDC (`change_type`), TS `strict`/`noUnusedLocals`.
4. **Wire de triggers en `.ai/AGENTS.md`** — indicar cuándo el agente debe cargar cada skill.
5. **Trial del skill de UI** (`ui-ux-pro-max-skill`) repo-scoped para tareas de UI; evaluar keep/drop; si aporta, extraer tokens/estilos hacia `packages/syntrix-ui`.
6. **(Fase B, diferido)** Registrar en el roadmap de `syntrix-ai` el trabajo de memoria/embeddings, referenciando `claude-mem`/hermes/ECC y reusando el stack SQLite-FTS5 (Limbo/Tantivy). No adoptar dependencias externas.
7. **Actualizar docs** — enlazar este ADR desde `.ai/AGENTS.md` y el plan maestro.

## 8. Validación

- Un skill portado **se dispara** en una sesión real de Kilo y produce el flujo esperado (plan → worktree → TDD → review) sin romper los gates existentes.
- Una tarea de UI usando `DESIGN.md` genera salida **consistente con shadcn/`syntrix-ui`**.
- `just lint` y `just test-rust` siguen verdes tras cambios de config (los skills/markdown no afectan compilación).
- El `rsync` a los servidores no se degrada (solo se añaden markdown; nada pesado en `.kilo/skill/`).

## 9. Consecuencias

- **Positivo**: formalizas tu proceso (skills versionados), estándar (spec Anthropic), sin lock-in de harness, sin tocar la arquitectura soberana del producto. Ganas un `DESIGN.md` reutilizable por agentes y por el propio catálogo de UI.
- **Negativo / costo**: mantenimiento de `.kilo/skill/` propio (portar ≠ instalar) y de la sección Instincts.
- **Sin cambios de producto**: `syntrix-ai` y sus principios permanecen intactos.

## 10. Disparadores de revisión

Reconsiderar si:
- Migras de harness (Kilo → OpenCode/Claude Code): reevaluar instalar superpowers/ECC/ui-skill como plugins nativos en vez de portados.
- Necesitas **memoria/embeddings de agente** de forma central: evaluar `claude-mem` (con cautela por token/worker) vs. implementación propia sobre Limbo/Tantivy en Fase B.
- El transfer de pesos/embeddings de modelos se vuelve central: ver también el ADR de red (`1783470696465`) sobre `iroh-blobs` como librería.

## 11. Fuera de alcance

- Instalar/ejecutar cualquier repo como dependencia del producto Syntrix.
- Configurar tooling en server-1/server-2.
- Adoptar un segundo harness de coding (decisión de plataforma separada).
- Implementación de las tareas de la §7 (requiere agente con permisos de escritura).
