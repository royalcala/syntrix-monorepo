---
title: "Conclusión final DeepSeek — Índices, correcciones y consenso de 6 IAs"
---

## 1. Índices: TanStack DB los maneja nativo. No escribirlos en redb.

### Qué encontré

TanStack DB tiene **`autoIndex`** en `CollectionConfig` y mantiene índices implícitos vía **differential dataflow (d2ts)**. No hay que implementar nada:

```typescript
// CollectionConfig ya soporta autoIndex (documentado en domain_map.yaml)
const invoicesCollection = createCollection({
  autoIndex: true,  // ← built-in, incremental, sub-ms
  getKey: (inv) => inv.id,
  schema: invoiceSchema,
  ...
})

// Esta query usa los índices automáticos del IVM engine:
useLiveQuery((q) =>
  q.from({ inv: invoicesCollection })
   .where(({ inv }) => eq(inv.customer_id, "cust-123"))
)
// → sub-ms, mantenido incrementalmente, sin código extra
```

**El IVM engine mantiene índices en memoria automáticamente.** Cuando entra un nuevo entry de iroh-docs al adapter, TanStack DB actualiza sus índices diferenciales. No hay que persistirlos a redb.

### Las 3 IAs del second-opinion coinciden (unánime)

| IA | Veredicto sobre índices en redb |
|----|--------------------------------|
| **Gemini** | "Jamás sincronices materializaciones o índices en una red P2P. Los índices son subproductos del log de eventos." |
| **Opus** | "Un índice es una vista derivada de los datos. Si sincas el índice como dato, tienes dos fuentes de verdad." |
| **Sonnet** | "Los índices deberían ser locales y derivados, nunca sincados. Cada peer reconstruye sus índices del event log." |
| **DeepSeek** | TanStack DB ya los genera en memoria vía d2ts. No escribirlos a redb. No sync P2P. Cero código. |

### Decisión final: índices en TanStack DB (memoria), no en redb

```
Flujo correcto:
  iroh-docs entry llega
    → adapter la inserta en TanStack DB collection
    → IVM engine (d2ts) actualiza índices automáticamente
    → useLiveQuery con where/join usa esos índices (sub-ms)
  
  NO hacer:
    → escribir _indexes/... en redb ❌
    → sync de índices P2P ❌
    → reconstruir índices manualmente ❌
```

---

## 2. Consenso de 6 IAs — las 3 correcciones unánimes al vision doc

### Corrección 1: Eliminar iroh-streams del stack final (Gemini, Opus, Sonnet)

Las 3 IAs del second-opinion + ChatGPT/Claude del first-approach coinciden: `iroh-streams` no debe estar en el stack final. Si se necesita web client, se expone un HTTP API simple desde Tauri (Axum), sin inventar un protocolo nuevo.

### Corrección 2: Índices locales, no sincados P2P (Gemini, Opus, Sonnet — UNÁNIME)

Resuelto arriba. TanStack DB los maneja nativo.

### Corrección 3: Schema versioning desde el día 1 (Opus, Sonnet)

Con 4 namespaces y entries inmutables en iroh-docs, cada entry debe llevar `schemaVersion`. El adapter aplica migraciones al vuelo en `deserializeEntry()`:

```typescript
function deserializeEntry(raw: RawEntry): Invoice {
  const v = JSON.parse(raw.value)
  if (v.schemaVersion === 1) { /* migrar v1 → v2 */ }
  if (v.schemaVersion === 2) { /* migrar v2 → v3 */ }
  return invoiceSchema.parse(v)  // Zod valida + transforma
}
```

---

## 3. Lo que el vision doc hace bien (validado por las 6 IAs)

| Decisión | Validación |
|----------|-----------|
| EntityDefinition metadata-driven | ✅ "La decisión más importante del documento y es la correcta" (Opus) |
| FieldRegistry separado de entidades | ✅ "Cuando agregues 50 entidades, el registry te ahorra mantenimiento" (ChatGPT) |
| IDs con prefijo de sucursal (`A-0042-XA1`) | ✅ "Brillante, elimina necesidad de secuenciador central" (Gemini) |
| Snapshots periódicos en iroh-docs | ✅ "Salva la RAM y el tiempo de inicio" (Gemini) |
| Escape hatch para entidades no-grid | ✅ "Demuestra madurez" (Opus) |
| Glide Data Grid como grid principal | ✅ Evaluado originalmente; reemplazado por TanStack Table + shadcn (mejor integración, mobile-first) |

---

## 3.1 Observaciones de Gemini (ronda 3) — corregir ahora

### Observación 1: Adapter debe bufferdear eventos batch (reconexión offline)

Cuando un peer vuelve online después de 1 semana offline, iroh-docs sync puede mandar 5,000+ entries en segundos. El adapter debe **debouncear o bufferear** las escrituras a TanStack DB para no saturar las mutaciones y congelar la UI.

```typescript
// En el adapter sync():
let batch: Entry[] = [];
let timeout: ReturnType<typeof setTimeout> | null = null;

function flushBatch() {
  if (batch.length === 0) return;
  begin();
  for (const entry of batch) write({ type: 'insert', value: deserializeEntry(entry) });
  commit();
  batch = [];
}

listen('data-changed', (event) => {
  batch.push(event.entry);
  if (batch.length >= 100) { flushBatch(); return; }  // flush cada 100
  if (timeout) clearTimeout(timeout);
  timeout = setTimeout(flushBatch, 50);  // o cada 50ms
});
```

### Observación 2: Separar `display_id` de `document_key`

El ID `A-0042-XA1` es el **folio** (lo que ve el usuario). La **key en iroh-docs** debe usar HLC para orden causal:

```
Display ID (UI):     A-0042-XA1
Document Key (redb): invoices/A-0042-XA1/events/{hlc_ts}:{hlc_count}:{node}

→ El display_id es inmutable y human-readable.
→ La key usa HLC para ordenamiento y resolución de conflictos.
→ Si dos peers generan el mismo counter por error, el HLC desempata.
```

### Observación 3: Eventos granulares por campo, no por fila

Si dos peers editan campos distintos del mismo registro offline, mandar la fila completa en un solo `commit_event` causa LWW y aplasta uno de los cambios. **Cada campo editado debe ser su propio evento.**

```typescript
// ❌ Mal: evento de fila completa
commit_event("invoice.updated", JSON.stringify({ id: "A-0042-XA1", status: "paid", amount: 200 }))

// ✅ Bien: eventos granulares por campo
commit_event("invoice.field_updated", JSON.stringify({ id: "A-0042-XA1", field: "status", value: "paid" }))
commit_event("invoice.field_updated", JSON.stringify({ id: "A-0042-XA1", field: "amount", value: 200 }))
```

Esto aplica tanto al grid (onCellEdited) como al detail panel (onBlur por campo). El event sourcing ya garantiza que ambos cambios sobreviven porque tocan campos distintos.

---

## 4. Lo que se incorpora ahora vs. después

### Ahora (afectan arquitectura desde el día 1)

| Item | Razón |
|------|-------|
| Estructura de keys con snapshots | `events/{hlc}` + `snapshots/{hlc}` bajo cada record. Determina qué queries son posibles. |
| Formato de IDs (`A-0042-XA1`) | Una vez en producción, cambiar IDs es migración mayor. |
| `schemaVersion` en cada entry | Sin esto, el primer cambio de schema rompe peers offline. |
| AutoIndex de TanStack DB | Ya está built-in. Se configura en `createCollection()`. |
| Índices en memoria (no en redb) | Decisión de arquitectura que evita problemas de consistencia. |

### Después (no afectan arquitectura, se agregan sin romper nada)

| Item | Razón |
|------|-------|
| Workflow engine | Se agrega como plugin en `EntityDefinition.workflow` |
| Plugin system | El `FieldTypePlugin` ya está diseñado para aceptar plugins |
| Kanban, calendar, gantt, map | Son vistas adicionales en `EntityDefinition.views` |
| Report builder | Es una entidad más con su propio `override.fullPage` |
| Field-level permissions | Es una propiedad adicional en `FieldConfig.permissions` |
| i18n | Las strings ya pasan por `t()`, solo se agregan archivos de traducción |
| Row-level security | Es una función en `EntityDefinition.permissions.canView` |
| Multi-moneda, multi-país | Son field types nuevos en el registry |

---

## 5. Principio rector (validado por las 6 IAs)

> "Menos infraestructura, más ERP. El valor para el usuario está en facturas, inventario y reportes — no en si el log se sincroniza por un protocolo perfecto." — Consenso de 4 IAs, primera ronda.

> "Este documento puede convertirse en scope creep disfrazado de planificación. Las únicas decisiones que afectan el MVP son la estructura de keys y el formato de IDs." — Sonnet.

**Lo que determina el éxito o fracaso del producto no es el vision doc — es que el adapter iroh-docs → TanStack DB funcione con datos reales, el grid scrollee a 60 FPS con 10,000 filas, y los permisos por namespace bloqueen payroll de sales a nivel de red.**