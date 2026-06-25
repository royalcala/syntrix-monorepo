---
title: "Syntrix — Plan de UI Perfeccionada"
---

## Diagnóstico

> **Estado actual:** Fases 1 y 4 completadas. Fases 2, 3, 6 parciales. Fases 5, 9, 10, 11 pendientes.

| Componente | Estado actual | Estado deseado |
|-----------|--------------|----------------|
| Grid | Datos planos, sin formato condicional, sin ordenamiento visual | Celdas con color por estado, ordenamiento clickeable, resize de columnas, selección múltiple |
| Detail Panel | Tabs básicos, inputs de texto | Formularios con Select, DatePicker, RelationPicker reales. Loading/empty/error states |
| Toolbar | "+ Nuevo" + contador | Vistas guardadas, filtros avanzados, búsqueda, exportar |
| Sidebar | Lista fija de entidades | Colapsable, favoritos, búsqueda de entidades, badge de conteo |
| Crear registro | Inline en el grid (solo campos texto) | Modal/panel según complejidad, con todos los field types funcionando |
| Estados | Solo loading/empty básico | Loading skeleton, empty con CTA, error con retry, success toast |
| Responsive | Solo desktop | Sidebar → bottom nav en mobile, detail panel → full screen sheet |
| Keyboard | Nada | Navegación con flechas, Enter para abrir, Escape para cerrar, Ctrl+K búsqueda |

---

## Fase 1: Field Types completos

Cada field type debe renderizarse correctamente en **grid** y en **detail panel**. No placeholders.

### 1.1 Text

- **Grid:** texto simple, truncado con ellipsis si excede ancho
- **Detail:** `<Input>` de shadcn
- **Estados:** empty → "—", loading → skeleton, error → borde rojo + mensaje

### 1.2 Number

- **Grid:** alineado a la derecha, formato de miles (1,234)
- **Detail:** `<Input type="number">` con step configurable
- **Validación:** min, max, integer

### 1.3 Currency

- **Grid:** `$1,234.56`, color rojo si es negativo
- **Detail:** `<Input>` con prefijo de moneda configurable
- **Formato:** según locale (MXN, USD, EUR)

### 1.4 Date

- **Grid:** `15 jun 2026`, relativo si < 7 días ("hace 3 días")
- **Detail:** `<DatePicker>` (popover con calendario)
- **Formatos:** short, long, relative

### 1.5 Select

- **Grid:** texto de la opción seleccionada
- **Detail:** `<Select>` de shadcn (dropdown nativo)
- **Opciones:** definidas en `field.options`

### 1.6 Status

- **Grid:** badge de color (`draft`=gris, `open`=azul, `paid`=verde, `cancelled`=rojo)
- **Detail:** `<Select>` con opciones coloreadas
- **Transiciones:** según workflow de la entidad

### 1.7 Relation

- **Grid:** nombre del registro relacionado (lookup), clickeable para navegar
- **Detail:** `<Command>` (combobox con búsqueda) para seleccionar de la entidad relacionada
- **Resolución:** `relationMaps` precomputado (O(1), no llamadas por celda)

### 1.8 Boolean

- **Grid:** ícono ✓ / ✗ o toggle
- **Detail:** `<Switch>` o `<Checkbox>` de shadcn

### 1.9 Indicador de Estado de Red (Sync)

- **Prioridad Crítica:** Mover de Fase 10 a Fase 1.
- **UI:** Pulso verde (conectado), gris (desconectado), o indicando "N eventos pendientes" en el sidebar/toolbar.
- **Feedback:** Asegura al usuario que sus datos están seguros localmente.

---

## Fase 2: Grid Avanzado

### 2.1 Ordenamiento

- Click en header → ordena ascendente
- Segundo click → descendente
- Tercer click → sin orden
- Indicador visual (flecha) en header activo

### 2.2 Filtros

- Por field type: texto→contiene, número→rango, fecha→entre, status→igual
- Filtros activos visibles como chips en toolbar
- Botón "Limpiar filtros"

### 2.3 Columnas

- Resize arrastrando borde del header
- Reordenar arrastrando header
- Menú "Columnas" en toolbar para mostrar/ocultar

### 2.4 Selección

- Click en row marker → selecciona fila
- Ctrl+click → multi-selección
- Shift+click → rango
- Acciones en lote: eliminar seleccionados, cambiar estado

### 2.5 Formato condicional

```typescript
// Por field type "status"
status: {
  draft: { bg: "muted", text: "muted-foreground" },
  open: { bg: "blue-100 dark:blue-900", text: "blue-700 dark:blue-300" },
  paid: { bg: "green-100 dark:green-900", text: "green-700 dark:green-300" },
  cancelled: { bg: "red-100 dark:red-900", text: "red-700 dark:red-300" },
}
```

### 2.6 Row height

- Compacto (28px), Normal (36px), Cómodo (48px)
- Selector en toolbar

### 2.7 Paginación / Infinite scroll

- Virtualización: TanStack Table con scroll nativo
- Si hay +10,000 filas → mostrar contador y opción de cargar más
- No paginación tradicional (el usuario scrollea libre)

---

## Fase 3: Detail Panel Completo

### 3.1 Layout responsive

- **Desktop:** slide-in panel derecho, 480px, con resize handle
- **Tablet:** slide-in panel derecho, 380px
- **Mobile:** bottom sheet, altura 80vh, swipe para cerrar

### 3.2 Tabs dinámicos

- "Datos": siempre presente, formulario del registro
- Tabs de relaciones: definidos en `entity.detail.tabs`. Ej: cliente → "Facturas" (sub-grid)
- "Historial": log de eventos de iroh-docs para este registro (exclusivo de `syntrix-client`, no implementado en `syntrix-admin`)

### 3.3 Formulario (Tab Datos)

- Campos organizados en grupos (si `field.group` está definido)
- Tooltip de ayuda en campos complejos
- Indicador de campo requerido (*)
- Botón "Guardar" explícito + Ctrl+Enter
- Discard changes dialog si hay cambios sin guardar

### 3.4 Sub-grids (Tab Relaciones)

- Si un cliente tiene facturas: sub-grid embebido en el tab
- Mismo EntityGrid, filtrado por `where(eq(invoice.customer_id, row.id))`
- Sin detail panel anidado (solo grid)

### 3.5 Navegación

- Flechas arriba/abajo → navegar entre registros sin cerrar el panel
- Ctrl+→ / Ctrl+← → tabs
- Escape → cerrar panel

### 3.6 Estados

- **Loading:** skeleton del formulario (bloques grises animados)
- **Empty:** "Este registro no tiene datos" con botón "Crear"
- **Error:** "No se pudo cargar el registro" con botón "Reintentar"
- **Saved:** toast "Cambios guardados" (sonner)

---

## Fase 4: Crear / Editar Registros

### 4.1 Estrategia Unificada (Detail Panel)

Para el MVP, se elimina la creación *inline* (fila vacía en el grid) para reducir complejidad por validaciones cruzadas y relaciones obligatorias. **Toda** creación de registros se hará desde el Detail Panel.

### 4.2 Detail panel create

- Mismo componente que edit, modo `isCreate=true`
- Foco automático en el primer campo
- Sin historial (no existe aún)
- Sin tabs de relaciones (no existe aún)
- Botón "Crear" primario, "Cancelar" secundario
- Tab para siguiente campo, Enter para confirmar (dentro del formulario)

### 4.4 Validación

- En tiempo real (onBlur): mostrar error debajo del campo
- Al intentar guardar: validar todos los campos, mostrar errores
- Campos requeridos: borde rojo si vacíos

---

## Fase 5: Vistas Guardadas

### 5.1 Qué es una vista

Una vista es una **query guardada** sobre una entidad. Define:
- Filtros aplicados
- Columnas visibles y su orden
- Ordenamiento
- Altura de fila
- Nombre y visibilidad (privada/compartida)

### 5.2 UI

- Dropdown en toolbar: "Todas", "Pendientes", "Pagadas"...
- "Guardar vista actual" → modal para nombrarla
- "Gestionar vistas" → panel para renombrar, reordenar, eliminar
- Vistas predefinidas por entidad (definidas en `entity.views`)

### 5.3 Implementación

```typescript
interface ViewDefinition {
  id: string;
  label: string;
  filters: FilterRule[];
  sort: { field: string; dir: "asc" | "desc" }[];
  visibleColumns: string[];
  rowHeight?: "compact" | "normal" | "comfortable";
  isDefault?: boolean;
}
```

Vistas guardadas en IndexedDB local (no sync P2P). Las predefinidas viven en `entity.views`.

---

## Fase 6: Búsqueda Global (Command Palette)

### 6.1 Activación

- **Ctrl+K** / **Cmd+K** → abre command palette
- Click en ícono de lupa en sidebar

### 6.2 Funcionalidad

```
┌─────────────────────────────────────┐
│ 🔍 "juan"                           │
│                                     │
│ Clientes                            │
│   Juan Pérez — juan@email.com       │
│   Juan Martínez — premium           │
│                                     │
│ Facturas                            │
│   F-1042 — Juan Pérez — $5,200     │
│                                     │
│ Acciones                            │
│   + Nueva factura    ⌘N             │
│   + Nuevo cliente                   │
│   Ir a → Productos                  │
│   Ir a → Dashboard                  │
└─────────────────────────────────────┘
```

### 6.3 Implementación

- Motor de búsqueda **Tantivy** (Rust, embebido en el proceso Tauri)
- Se alimenta automáticamente desde el indexador (`upsert_document` en `indexes.rs`)
- Soporta: **fuzzy search** ("Akme" → "Acme"), **ranking BM25**, **snippets con highlights**
- Comando Tauri: `invoke("search_entity", { query, entities?, limit? })`
- Debounce de 150ms en el input del frontend
- Resultados agrupados por entidad + acciones
- Navegación con flechas, Enter para abrir

---

## Fase 7: Atajos de Teclado

| Atajo | Acción | Contexto |
|-------|--------|----------|
| `Ctrl+K` | Command palette | Global |
| `Ctrl+N` | Nuevo registro | Grid |
| `Enter` | Abrir detail panel | Grid (fila seleccionada) |
| `Escape` | Cerrar detail panel / Cancelar edición | Detail |
| `Ctrl+Enter` | Guardar cambios | Detail (editando) |
| `Ctrl+→` | Siguiente tab | Detail |
| `Ctrl+←` | Tab anterior | Detail |
| `Flechas` | Navegar celdas | Grid |
| `Tab` | Siguiente campo | Inline edit / Form |
| `F2` | Editar celda | Grid |
| `Delete` | Eliminar filas seleccionadas | Grid |
| `Ctrl+Z` | Deshacer | Grid (optimistic rollback) |

---

## Fase 8: Feedback y Estados Visuales

### 8.1 Loading states

- **Grid cargando:** skeleton (filas grises animadas)
- **Detail cargando:** skeleton del formulario
- **Relación cargando:** spinner en el combobox

### 8.2 Empty states

- **Grid vacío:** ilustración + "Aún no hay {entidad}" + botón "Crear primer registro"
- **Detail vacío:** "Seleccioná un registro del grid"
- **Búsqueda sin resultados:** "No se encontraron resultados para '{query}'" + sugerencias

### 8.3 Error states

- **Error de carga:** "No se pudo cargar {entidad}" + botón "Reintentar"
- **Error de guardado:** toast de error + mantener datos en formulario
- **Error de sync:** badge en sidebar indicando desconexión

### 8.4 Toasts (sonner)

- **Éxito:** "Cliente creado", "Factura actualizada"
- **Error:** "No se pudo guardar: conflicto de edición"
- **Info:** "Sincronizando con 3 peers"

### 8.5 Skeleton components

```tsx
<Skeleton className="h-4 w-[250px]" />       // texto
<Skeleton className="h-8 w-full" />           // input
<Skeleton className="h-32 w-full" />          // grid
```

---

## Fase 9: Responsive

### 9.1 Breakpoints

- **Desktop (≥1024px):** sidebar fijo 240px, grid + detail panel
- **Tablet (768-1023px):** sidebar colapsado (íconos), grid full-width, detail panel overlay
- **Mobile (<768px):** bottom nav en vez de sidebar, grid ocupa todo, detail → full screen sheet

### 9.2 Sidebar → Bottom Nav (mobile)

```
Desktop:                            Mobile:
┌─ Sidebar ─┐ ┌─ Grid ──────────┐   ┌─ Grid (full width) ──────────┐
│ Clientes  │ │                  │   │                              │
│ Facturas  │ │                  │   │                              │
│ Productos │ │                  │   └──────────────────────────────┘
│ Órdenes   │ │                  │   ┌─ Bottom Nav ─────────────────┐
└───────────┘ └──────────────────┘   │ 🏠  📄  📦  🛒  ⚙️         │
                                     └──────────────────────────────┘
```

### 9.3 Detail Panel → Sheet (mobile)

- En mobile, el detail panel ocupa 100% de la pantalla
- Swipe hacia abajo para cerrar
- Header sticky con botón "← Volver"

---

## Fase 10: Pulido Visual

### 10.1 Transiciones

- Detail panel: slide-in desde derecha (200ms ease-out)
- Modal: fade-in + scale (150ms ease-out)
- Toast: slide-up desde abajo (300ms)
- Hover en filas: transición de background (100ms)

### 10.2 Micro-interacciones

- Contador de registros: animación al cambiar
- Badge de inbox: pulso cuando hay nuevos
- Check de guardado: ícono que hace check animado

### 10.3 Consistencia tipográfica

- Headers: 13px semibold
- Celdas: 13px regular
- Labels: 11px medium, muted-foreground
- Badges: 11px medium

### 10.4 Espaciado

- Grid: 8px padding horizontal en celdas
- Detail: 16px padding en formulario
- Toolbar: 12px padding vertical
- Sidebar: 8px entre items

---

## Plan de trabajo reordenado

| Semana | Fase | Entregable | Estado |
|--------|------|-----------|--------|
| 1 | Fase 1 | 8 field types completos en grid + detail | ✅ |
| 2 | Fase 2 + 3 | Grid avanzado (sort, filter, columns, selection) + Detail panel | ⚠️ Parcial |
| 3 | Fase 4 + 5 | Crear/editar registros + vistas guardadas | ✅ / ⚠️ |
| 4 | Fase 6 + 7 | Command palette + atajos de teclado | ⚠️ Parcial |
| 5 | Fase 8 + 9 | Estados visuales + responsive | ⚠️ / ❌ |
| 6 | Fase 10 | Pulido visual, transiciones, micro-interacciones | ⚠️ |

**Total: 6 semanas para UI nivel Airtable.**

Después de esto, Fase 3 del plan original (workflow, dashboard, exportar, búsqueda).

---

## Lo que NO tocamos hasta que la UI esté perfecta

- Workflow de factura
- Dashboard
- Exportar CSV
- Números de factura
- Adjuntar archivos
- Notificaciones
- Cualquier feature de negocio

---

## Fase 11 (futuro): Vistas alternativas (Kanban, Calendar, Gallery)

Implementadas como `layout` alternativo en `EntityDefinition.views`. Misma entidad, misma colección, misma toolbar. Solo cambia el renderer.

| Vista | Caso de uso | Prioridad |
|-------|------------|-----------|
| **Kanban** | Pipeline de ventas, estados de órdenes | Fase 4 — apenas haya CRM |
| **Calendar** | Fechas de entrega, vencimientos, citas | Fase 4 — con campo `date` |
| **Gallery** | Catálogo visual de productos | Fase 5 — cuando haya imágenes |

**No incluir:** Gantt (construcción, nicho), Mapa (reparto, nicho), Timeline (el historial del detail panel lo cubre).

---

## ¿Acuerdo?

Este plan asume que paramos todo desarrollo de features y nos enfocamos 6 semanas en pulir la UI. Cada fase tiene un entregable concreto y verificable. ¿Ajustamos algo o arrancamos Fase 1?