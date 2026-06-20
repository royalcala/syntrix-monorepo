# Syntrix Workspace — Diseño de UI

## Filosofía

No diseñamos módulos como páginas separadas. Todo es una **entidad** con un **grid** y un **panel de detalle**. El usuario navega entre entidades, no entre pantallas.

Inspiración:
- **Airtable** → grid rápido, field types con comportamiento, vistas configurables
- **Notion/Linear** → panel lateral al hacer click, backlinks, command palette
- **Odoo** → acciones de negocio por entidad
- **Obsidian** → offline-first, datos locales

---

## Arquitectura de componentes

```
┌─ Workspace ─────────────────────────────────────────────┐
│ ┌─ Sidebar ──────┐ ┌─ Main Area ───────────────────────┐│
│ │                 │ │ ┌─ Toolbar ─────────────────────┐ ││
│ │ 🔍 Search       │ │ │  Vistas  │  Filtros  │  +Nuevo│ ││
│ │                 │ │ └───────────────────────────────┘ ││
│ │ ★ Favoritos     │ │ ┌─ Grid ────────────────────────┐ ││
│ │                 │ │ │                               │ ││
│ │ Clientes        │ │ │ TanStack Table + shadcn        │ ││
│ │ Facturas        │ │ │ (DOM, shadcn compatible)       │ ││
│ │ Productos       │ │ │                               │ ││
│ │ Órdenes         │ │ │                               │ ││
│ │ Inventario      │ │ └───────────────────────────────┘ ││
│ │                 │ │                                    ││
│ └─────────────────┘ └────────────────────────────────────┘│
│                          ┌─ Detail Panel (slide-in) ─────┐│
│                          │ ┌─ Tabs ────────────────────┐ ││
│                          │ │ Datos │ Facturas │ Hist.  │ ││
│                          │ └───────────────────────────┘ ││
│                          │ ┌─ Form / Sub-grid ─────────┐ ││
│                          │ │                           │ ││
│                          │ └───────────────────────────┘ ││
│                          └───────────────────────────────┘│
└──────────────────────────────────────────────────────────┘
```

Tres zonas:
1. **Sidebar** — navegación entre entidades (clientes, facturas, productos...)
2. **Grid** — la entidad activa en formato tabla (TanStack Table + shadcn)
3. **Detail Panel** — se abre al hacer click en una fila, muestra/edita el registro

---

## La entidad como concepto unificador

Cada espacio operativo es una **entidad**. Una entidad define:

```typescript
interface EntityDefinition {
  id: string;                    // "customers", "invoices", "products"
  label: string;                 // "Clientes", "Facturas"
  icon: string;                  // lucide icon name
  collection: Collection<any>;   // TanStack DB collection
  fields: FieldDefinition[];     // columnas del grid
  views: ViewDefinition[];       // vistas guardadas (filtros + columnas)
  detail: DetailConfig;          // qué mostrar en el panel lateral
  actions: EntityAction[];       // acciones de negocio (crear, exportar, etc.)
  searchFields: string[];        // campos indexados para búsqueda
}
```

### FieldDefinition (columna del grid)

```typescript
interface FieldDefinition {
  key: string;                   // "name", "amount", "status"
  label: string;                 // "Nombre", "Monto"
  type: FieldType;               // text, number, date, select, currency, relation
  width: number;                 // ancho por defecto en px
  editable: boolean;             // ¿se puede editar inline?
  sortable: boolean;
  // Renderizado en el grid
  renderCell: (value: any, row: any) => ReactNode;  // TanStack Table cell
  // Renderizado en el panel de detalle
  renderDetail?: (value: any, row: any, onChange: (v: any) => void) => ReactNode;
  // Validación
  validate?: (value: any) => string | null;
}
```

### FieldType (comportamiento del campo)

```typescript
type FieldType =
  | "text"
  | "number"
  | "currency"
  | "date"
  | "select"        // dropdown con opciones
  | "relation"      // link a otra entidad (ej: invoice.customer_id → customer.name)
  | "status"        // badge de color (draft/open/paid/cancelled)
  | "boolean"
  | "email"
  | "phone"
  | "computed";     // calculado, no editable
```

Cada `FieldType` sabe:
- **renderizarse** en el grid (canvas cell)
- **renderizarse** en el detail panel (React component)
- **editarse** (input, select, datepicker, etc.)
- **validarse** (reglas de negocio)
- **filtrarse** (operadores disponibles: equals, contains, gt, lt, between)

### ViewDefinition (vista guardada)

```typescript
interface ViewDefinition {
  id: string;
  label: string;                 // "Todas", "Morosos", "Premium"
  filters: FilterRule[];         // [{ field: "status", op: "eq", value: "open" }]
  sort: { field: string; dir: "asc" | "desc" }[];
  visibleColumns: string[];      // qué columnas mostrar
  groupBy?: string;              // campo para agrupar filas
}
```

---

## TanStack Table + shadcn como motor del grid

## TanStack Table + shadcn como motor del grid

DOM-based → celdas `<td>` con Tailwind, theming shadcn nativo vía CSS variables. TanStack Table es headless — maneja sorting, filtering, pagination. El renderizado es 100% controlable.

Ventajas:
- **shadcn nativo** → Table, TableRow, TableCell con Tailwind
- **TanStack ecosystem** → misma familia que TanStack DB y TanStack Router
- **Headless** → control total del markup, accesibilidad
- **Mobile-first** → las filas son DOM real, scrolleables, cliqueables

### Adaptador: TanStack DB collection → TanStack Table

```typescript
function EntityGrid({ entity }: { entity: EntityDefinition }) {
  const { data } = useLiveQuery((q) =>
    q.from({ row: entity.collection })
     .select(...entity.fields.map(f => ({ [f.key]: row[f.key] })))
  );

  const columns: GridColumn[] = entity.fields.map(f => ({
    id: f.key,
    title: f.label,
    width: f.width,
  }));

  const getCellContent = ([col, row]: Item): GridCell => {
    const field = entity.fields[col];
    const value = data[row]?.[field.key];
    return field.renderCell(value, data[row]);
  };

  const onCellEdited = (cell: Item, newValue: GridCell) => {
    const field = entity.fields[cell[0]];
    const row = data[cell[1]];
    entity.collection.update(row.id, { [field.key]: newValue.data });
  };

  return (
    <DataEditor
      columns={columns}
      rows={data.length}
      getCellContent={getCellContent}
      onCellEdited={onCellEdited}
      onRowClicked={(row) => openDetail(row)}
      // ... más props
    />
  );
}
```

---

## Detail Panel (panel lateral)

Al hacer click en una fila del grid, se abre un panel lateral con tabs:

```typescript
interface DetailConfig {
  tabs: DetailTab[];
  width?: number;  // ancho del panel en px (default 480)
}

interface DetailTab {
  key: string;          // "data", "related", "history"
  label: string;        // "Datos", "Facturas", "Historial"
  icon?: string;
  render: (row: any, entity: EntityDefinition) => ReactNode;
}
```

### Tab "Datos" — formulario del registro

Muestra todos los campos de la entidad en modo edición. Cada `FieldType` tiene su `renderDetail`:

```typescript
// Ejemplo: field type "relation" (customer_id → customer.name)
renderDetail: (value, row, onChange) => (
  <RelationPicker
    value={value}
    targetEntity="customers"     // entidad relacionada
    displayField="name"           // campo a mostrar
    onChange={onChange}
  />
)
```

### Tab "Facturas" (relaciones)

Si el registro es un cliente, esta tab muestra un sub-grid con las facturas de ese cliente. Es una query con `where(eq(invoice.customer_id, row.id))`.

### Tab "Historial"

Muestra el log de cambios del registro. Cada entry de iroh-docs con el key prefix de esta entidad + id.

---

## Entidades del MVP

| Entidad | Fields principales | Detail Tabs |
|---------|-------------------|-------------|
| **Clientes** | nombre, tax_id, email, teléfono, dirección | Datos, Facturas, Pedidos, Historial |
| **Facturas** | id, cliente (relation), fecha, total, status | Datos, Items, Historial |
| **Productos** | nombre, sku, precio, unidad, categoría | Datos, Historial |
| **Órdenes** | id, cliente, items, fecha, status | Datos, Items, Historial |

Todas usan el mismo `EntityGrid` y el mismo `DetailPanel`. Solo cambia la definición de `EntityDefinition`.

---

## Flujo de usuario típico

```
1. Usuario abre Syntrix
   → Sidebar muestra entidades: Clientes, Facturas, Productos, Órdenes
   → Grid muestra la última entidad activa (o Dashboard)

2. Naviga a "Facturas"
   → Grid carga invoicesCollection via useLiveQuery
   → Columnas: ID, Cliente, Fecha, Total, Status
   → Cada celda se renderiza según su FieldType

3. Click en fila "INV-1042"
   → Detail Panel se abre a la derecha
   → Tab "Datos": formulario con todos los campos editables
   → Tab "Items": sub-grid con los items de esta factura

4. Edita el campo "Status" (select: draft → open)
   → optimistic update en TanStack DB
   → commit_event("invoice.updated") → iroh-docs
   → sync a otros peers

5. Ctrl+K → Command Palette
   → "Crear factura" → abre detail panel en modo create
   → "Ir a clientes" → navega a la entidad clientes
   → "Buscar Juan" → búsqueda global
```

---

## Componentes compartidos

| Componente | Descripción | Dónde se usa |
|-----------|-------------|-------------|
| `EntityGrid` | Grid para cualquier entidad | Todas las entidades |
| `DetailPanel` | Panel lateral con tabs | Click en cualquier fila |
| `FieldRenderer` | Renderiza un field type en el grid (canvas) | `renderCell` |
| `FieldEditor` | Renderiza un field type en el detail (React) | `renderDetail` |
| `RelationPicker` | Selector de entidad relacionada | Fields tipo "relation" |
| `CommandPalette` | Búsqueda global + comandos (Ctrl+K) | App shell |
| `ViewSelector` | Dropdown de vistas guardadas | Toolbar del grid |
| `StatusBadge` | Badge de color para campos "status" | Grid + Detail |

---

## Ejemplo: definición de la entidad "Facturas"

```typescript
const invoicesEntity: EntityDefinition = {
  id: "invoices",
  label: "Facturas",
  icon: "receipt",
  collection: invoicesCollection,
  fields: [
    {
      key: "id", label: "Folio", type: "text", width: 120,
      editable: false, sortable: true,
      renderCell: (v) => ({ kind: GridCellKind.Text, data: v, displayData: v, allowOverlay: false }),
    },
    {
      key: "customer_id", label: "Cliente", type: "relation", width: 200,
      editable: true, sortable: true,
      renderCell: (v, row) => ({
        kind: GridCellKind.Text,
        data: v,
        displayData: getCustomerName(v),  // resuelve el nombre del cliente
        allowOverlay: true,
      }),
      renderDetail: (v, row, onChange) => (
        <RelationPicker value={v} targetEntity="customers" displayField="name" onChange={onChange} />
      ),
    },
    {
      key: "date", label: "Fecha", type: "date", width: 120,
      editable: true, sortable: true,
      renderCell: (v) => ({
        kind: GridCellKind.Text,
        data: v.toISOString(),
        displayData: v.toLocaleDateString(),
        allowOverlay: false,
      }),
    },
    {
      key: "amount", label: "Total", type: "currency", width: 120,
      editable: false, sortable: true,
      renderCell: (v) => ({
        kind: GridCellKind.Number,
        data: v,
        displayData: `$${v.toFixed(2)}`,
        allowOverlay: false,
      }),
    },
    {
      key: "status", label: "Estado", type: "status", width: 100,
      editable: true,
      renderCell: (v) => ({
        kind: GridCellKind.Text,
        data: v,
        displayData: v,
        allowOverlay: true,
        themeOverride: statusTheme(v),  // color según estado
      }),
      renderDetail: (v, row, onChange) => (
        <Select value={v} onValueChange={onChange}>
          <SelectItem value="draft">Borrador</SelectItem>
          <SelectItem value="open">Abierta</SelectItem>
          <SelectItem value="paid">Pagada</SelectItem>
          <SelectItem value="cancelled">Cancelada</SelectItem>
        </Select>
      ),
    },
  ],
  views: [
    { id: "all", label: "Todas", filters: [], sort: [{ field: "date", dir: "desc" }], visibleColumns: ["id","customer_id","date","amount","status"] },
    { id: "open", label: "Pendientes", filters: [{ field: "status", op: "eq", value: "open" }], sort: [{ field: "date", dir: "desc" }], visibleColumns: ["id","customer_id","date","amount","status"] },
    { id: "paid", label: "Pagadas", filters: [{ field: "status", op: "eq", value: "paid" }], sort: [{ field: "date", dir: "desc" }], visibleColumns: ["id","customer_id","date","amount","status"] },
  ],
  detail: {
    tabs: [
      {
        key: "data", label: "Datos",
        render: (row) => <EntityForm entity={invoicesEntity} row={row} />,
      },
      {
        key: "items", label: "Items",
        render: (row) => <InvoiceItemsGrid invoiceId={row.id} />,
      },
      {
        key: "history", label: "Historial",
        render: (row) => <HistoryLog entity="invoices" recordId={row.id} />,
      },
    ],
  },
  actions: [
    { id: "create", label: "Nueva factura", shortcut: "n", handler: () => openDetail(null, "create") },
    { id: "export", label: "Exportar CSV", handler: () => exportCSV(invoicesCollection) },
  ],
  searchFields: ["id", "customer_id"],
};
```

---

## Plan de implementación

### Fase 1: Core workspace (1-2 días)
1. Instalar `@tanstack/react-table` en ambos proyectos
2. Crear `EntityGrid` component (wrapper de DataEditor + TanStack DB)
3. Crear `DetailPanel` component (slide-in con tabs)
4. Crear `FieldRenderer` + `FieldEditor` para tipos básicos (text, number, date)
5. Definir `EntityDefinition` types

### Fase 2: Entidades del MVP (1 día)
6. `customersEntity` + grid + detail panel
7. `invoicesEntity` + grid + detail panel (con tab "Items")
8. `productsEntity` + grid + detail panel

### Fase 3: UX avanzada (1-2 días)
9. `EntityViewSelector` (vistas guardadas con filtros)
10. `CommandPalette` (Ctrl+K, búsqueda global)
11. `RelationPicker` (selector de entidad relacionada)
12. Edit inline en el grid (onCellEdited → TanStack DB update → commit_event)

---

## Dependencias nuevas

```json
{
  "@tanstack/react-table": "^8.21.2"
}
