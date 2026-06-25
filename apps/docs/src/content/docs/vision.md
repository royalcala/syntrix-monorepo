---
title: "Syntrix — La Nueva Matriz Soberana"
---

## Filosofía

Syntrix no es un ERP. Es un **Espacio Operativo Autónomo**: una plataforma de colaboración soberana donde cada colectivo — empresa, familia, equipo — tiene control absoluto sobre su información y su operación.

Este documento describe el destino del producto. No es el MVP — es la matriz completa que emerge cuando los nodos libres se sincronizan sin intermediarios.

Los 4 pilares que guían cada decisión:

1. **Soberanía Absoluta** — tus datos viven en tus dispositivos. Nadie más tiene acceso.
2. **Inmediatez Radical** — sin viajes a servidores. Cada clic ocurre en cero milisegundos.
3. **Sincronía Orgánica** — los nodos se entrelazan en silencio, sin que el usuario lo note.
4. **Resiliencia por Diseño** — sin internet, el espacio sigue operando. La dependencia se terminó.

---

## 1. Escala de la matriz

| Dimensión | MVP | Matriz completa |
|-----------|-----|-----------------|
| Espacios operativos | 4 (clientes, facturas, productos, órdenes) | **100+** (inventario, compras, nómina, contabilidad, proyectos, CRM...) |
| Registros por espacio | ~1,000 | **1M+** |
| Nodos por colectivo | 3-5 | **50-500** |
| Colectivos interconectados | 2-3 | **100+** (cadena nacional, red de franquicias) |
| Roles | 4 (admin, ventas, contabilidad, RH) | **20+** con matriz de permisos granular |
| Dispositivos por nodo | 1-2 | **3-5** (laptop, celular, tablet, punto de venta) |
| Nodos concurrentes | 3-5 | **50+** concurrentes leyendo/escribiendo |
| Idiomas | 1 (español) | **2+** (español, inglés) |
| Monedas | 1 (MXN) | **Multi-moneda** con tipos de cambio |
| Regímenes fiscales | 1 (México) | **Multi-país** (México, Colombia, Chile...) |

---

## 2. Espacios operativos de la matriz

```
┌─ Syntrix ─────────────────────────────────────────────────┐
│                                                            │
│  🏢 Relaciones              📊 Inteligencia                │
│  ├── Clientes               ├── Dashboards vivos            │
│  ├── Contactos              ├── Reportes configurables      │
│  ├── Oportunidades          ├── KPIs por nodo              │
│  └── Actividades            └── Exportación                │
│                                                            │
│  💰 Intercambios            📦 Recursos                    │
│  ├── Cotizaciones           ├── Productos                  │
│  ├── Pedidos                ├── Almacenes                  │
│  ├── Facturas               ├── Movimientos                │
│  ├── Notas de crédito       ├── Conteos físicos            │
│  └── Punto de Intercambio   └── Trazabilidad               │
│                                                            │
│  🛒 Abastecimiento          💳 Trazabilidad Financiera     │
│  ├── Proveedores            ├── Plan de cuentas            │
│  ├── Órdenes de compra      ├── Registros contables        │
│  ├── Recepción              ├── Balanzas                   │
│  └── Cuentas por pagar      └── Estados financieros        │
│                                                            │
│  👥 Colaboradores           🔧 Gobierno de la Matriz       │
│  ├── Nodos (dispositivos)   ├── Colectivo                  │
│  ├── Retribución            ├── Roles y llaves             │
│  ├── Presencia              ├── Nodos conectados           │
│  └── Expedientes            └── Parámetros                 │
│                                                            │
│  🔔 Señales                 🔌 Puentes                     │
│  ├── En la matriz           ├── Facturación electrónica    │
│  ├── Email                  ├── Bancos                     │
│  ├── Push                   ├── APIs externas              │
│  └── Webhooks               └── Conectores                 │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

---

## 3. Arquitectura de UI — el sistema completo

### 3.1 Design System

Un Design System propio con tokens de diseño que alimentan shadcn + TanStack Table:

```
design-system/
├── tokens/
│   ├── colors.ts          ← paleta de colores (light/dark)
│   ├── typography.ts      ← escala tipográfica
│   ├── spacing.ts         ← grid de espaciado (4px base)
│   ├── shadows.ts         ← sombras por elevación
│   └── radii.ts           ← bordes redondeados
├── components/
│   ├── ui/                ← shadcn primitives (Button, Input, Select...)
│   ├── data/              ← componentes de datos (EntityGrid, DetailPanel, Kanban...)
│   ├── layout/            ← AppShell, Sidebar, Toolbar
│   └── feedback/          ← Toast, Dialog, EmptyState, ErrorBoundary
└── patterns/
    ├── entity-page.tsx     ← patrón estándar para cualquier entidad
    ├── form-pattern.tsx    ← patrón de formularios (create/edit)
    └── list-detail.tsx     ← patrón maestro-detalle
```

### 3.2 FieldRegistry extendido (100+ field types)

El FieldRegistry crece con el producto. Cada field type es un módulo independiente:

```
fields/
├── registry.ts             ← registro central
├── types/
│   ├── text.ts             ← texto simple
│   ├── number.ts           ← numérico
│   ├── currency.ts         ← moneda con símbolo + formato
│   ├── date.ts             ← fecha con datepicker
│   ├── datetime.ts         ← fecha + hora
│   ├── select.ts           ← dropdown (opciones fijas)
│   ├── multiselect.ts      ← selección múltiple
│   ├── relation.ts         ← relación a otra entidad
│   ├── computed.ts         ← campo calculado (fórmula)
│   ├── status.ts           ← badge de estado con color
│   ├── boolean.ts          ← toggle / checkbox
│   ├── email.ts            ← email con validación
│   ├── phone.ts            ← teléfono con formato
│   ├── url.ts              ← link clickeable
│   ├── image.ts            ← imagen (thumbnail + lightbox)
│   ├── file.ts             ← archivo adjunto
│   ├── richtext.ts         ← texto enriquecido (Markdown/TipTap)
│   ├── color.ts            ← selector de color
│   ├── rating.ts           ← estrellas (1-5)
│   ├── progress.ts         ← barra de progreso
│   ├── barcode.ts          ← código de barras / QR
│   ├── signature.ts        ← firma digital
│   ├── location.ts         ← coordenadas / mapa
│   ├── tags.ts             ← etiquetas múltiples
│   ├── json.ts             ← objeto JSON (modo árbol)
│   ├── formula.ts          ← fórmula tipo Excel
│   ├── lookup.ts           ← búsqueda en otra entidad (autocomplete)
│   ├── rollup.ts           ← agregación de relación (SUM, COUNT, AVG)
│   ├── attachment.ts       ← galería de archivos
│   ├── timeline.ts         ← línea de tiempo
│   └── ...
└── plugins/                ← field types de terceros
    ├── sat/
    │   ├── rfc.ts          ← validación RFC México
    │   ├── cfdi-uuid.ts    ← UUID de factura electrónica
    │   └── regimen-fiscal.ts
    └── ...
```

**Cada field type implementa:**

```typescript
interface FieldTypePlugin {
  id: string;
  label: string;
  icon: string;
  // Grid rendering (canvas)
  grid: {
    renderCell: (value: any, options: CellOptions) => GridCell;
    getWidth?: (value: any) => number;  // ancho dinámico
  };
  // Detail rendering (React DOM)
  detail: {
    renderEditor: (props: FieldEditorProps) => ReactNode;
    renderViewer: (props: FieldViewerProps) => ReactNode;
  };
  // Validation
  validate: (value: any, context: ValidationContext) => ValidationResult;
  // Filtering
  filter: {
    operators: FilterOperator[];  // eq, neq, contains, gt, lt, between, in...
    renderFilter: (props: FilterProps) => ReactNode;
  };
  // Sorting
  sort: (a: any, b: any, dir: "asc" | "desc") => number;
  // Search
  search?: (value: any, query: string) => boolean;
  // Import/Export
  serialize?: (value: any) => string;
  deserialize?: (raw: string) => any;
}
```

### 3.3 EntityDefinition completo

```typescript
interface EntityDefinition {
  // Identidad
  id: string;
  label: string;
  labelPlural: string;
  icon: string;
  description?: string;
  module: ModuleId;  // "crm", "sales", "inventory", "accounting", "hr"

  // Datos
  collection: Collection<any>;
  fields: FieldConfig[];

  // Vistas
  views: ViewDefinition[];
  defaultView: string;

  // Panel de detalle
  detail: {
    tabs: DetailTab[];
    headerActions?: EntityAction[];   // acciones en el header del panel
    contextWidgets?: ContextWidget[]; // widgets en zona de contexto
  };

  // Acciones
  actions: EntityAction[];

  // Búsqueda
  search: {
    fields: string[];                 // campos indexados
    quickActions?: QuickAction[];     // "crear", "importar"
  };

  // Permisos
  permissions: {
    canView: PermissionCheck;
    canCreate: PermissionCheck;
    canEdit: PermissionCheck;
    canDelete: PermissionCheck;
    canExport: PermissionCheck;
    canImport: PermissionCheck;
  };

  // Visualización
  display: {
    primaryField: string;             // campo que identifica el registro (nombre, folio)
    secondaryField?: string;          // subtexto en selectores
    avatarField?: string;             // imagen/avatar
    badge?: (row: any) => BadgeConfig;  // badge en el grid
  };

  // Comportamiento
  behavior: {
    allowInlineCreate?: boolean;      // crear desde el grid sin abrir panel
    allowBulkEdit?: boolean;          // selección múltiple + edición masiva
    allowDuplicate?: boolean;         // duplicar registro
    requireConfirmationOnDelete?: boolean;
    softDelete?: boolean;             // papelera en vez de borrado físico
  };

  // Escape hatch
  override?: {
    grid?: React.ComponentType;       // grid personalizado (POS, calendario)
    detail?: React.ComponentType;     // detail personalizado
    fullPage?: React.ComponentType;   // página completa (dashboard, config)
  };

  // Workflow
  workflow?: WorkflowDefinition;      // estados + transiciones

  // Integraciones
  integrations?: {
    exportFormats?: ("csv" | "xlsx" | "pdf" | "xml")[];
    webhooks?: WebhookDefinition[];
    api?: ApiEndpoint[];
  };
}
```

### 3.4 Workflow Engine

Cada entidad puede tener un workflow de estados con transiciones y validaciones:

```typescript
interface WorkflowDefinition {
  initial: string;
  states: Record<string, WorkflowState>;
}

interface WorkflowState {
  label: string;
  color: string;
  transitions: WorkflowTransition[];
  onEnter?: Action[];   // acciones al entrar al estado
  onExit?: Action[];     // acciones al salir del estado
}

interface WorkflowTransition {
  to: string;
  label: string;
  guard?: (row: any) => boolean;          // condición
  validation?: ValidationRule[];           // validación requerida
  sideEffects?: Action[];                  // efectos secundarios
  permissions?: PermissionCheck;           // quién puede ejecutar
  requireComment?: boolean;                // requiere comentario
}
```

Ejemplo: workflow de factura

```typescript
{
  initial: "draft",
  states: {
    draft: {
      transitions: [
        { to: "sent", label: "Enviar", validation: [clientHasEmail] },
        { to: "cancelled", label: "Cancelar", permissions: canCancelInvoice },
      ]
    },
    sent: {
      transitions: [
        { to: "paid", label: "Registrar pago", sideEffects: [updateBalance, notifyAccountant] },
        { to: "overdue", label: "Marcar vencida", guard: (row) => row.dueDate < today },
      ]
    },
    paid: {
      transitions: [
        { to: "cancelled", label: "Anular", permissions: canCancelPaidInvoice, requireComment: true },
      ]
    },
  }
}
```

### 3.5 Vistas del workspace (más allá del grid)

No todo es grid+detail. El workspace soporta múltiples layouts:

```
Layouts disponibles:
├── grid             ← tabla (TanStack Table + shadcn) — 80% de las entidades
├── kanban           ← tablero kanban (oportunidades, proyectos)
├── calendar         ← calendario (entregas, citas)
├── gallery          ← galería de cards (productos con imágenes)
├── gantt            ← cronograma (proyectos, producción)
├── map              ← mapa (clientes, sucursales)
├── form             ← formulario (encuestas, captura rápida)
├── timeline         ← línea de tiempo (actividad, historial)
├── dashboard        ← dashboards con widgets
└── custom           ← componente React arbitrario (escape hatch)
```

Cada entidad define qué layouts soporta:

```typescript
entity: {
  views: [
    { id: "all", label: "Todas", layout: "grid", ... },
    { id: "kanban", label: "Kanban", layout: "kanban", groupBy: "status" },
    { id: "calendar", label: "Calendario", layout: "calendar", dateField: "dueDate" },
  ]
}
```

### 3.6 Búsqueda global (tipo Spotlight/Linear)

```
Ctrl+K → Command Palette

Resultados:
  ┌─────────────────────────────────────┐
  │ 🔍 "juan pérez"                     │
  │                                     │
  │ Clientes                            │
  │   Juan Pérez — juan@email.com       │
  │   Juan Martínez — Cliente premium   │
  │                                     │
  │ Facturas                            │
  │   INV-1042 — Juan Pérez — $5,200    │
  │   INV-0891 — Juan Pérez — $1,800    │
  │                                     │
  │ Acciones                            │
  │   + Nueva factura                   │
  │   + Nuevo cliente                   │
  │   Ir a → Facturas                   │
  │   Ir a → Dashboard                  │
  └─────────────────────────────────────┘
```

Implementación: índice de búsqueda en memoria (FlexSearch o Minisearch) actualizado en cada sync de TanStack DB.

---

## 4. Arquitectura de datos a escala

### 4.1 Estructura de entradas en iroh-docs

```
operational/
  {entity_type}/{record_id}/
    events/
      {hlc_timestamp}/
        {event_type}       ← valor: payload JSON del evento
    snapshots/
      {hlc_timestamp}      ← valor: estado completo del registro en ese momento
```

**Snapshots periódicos** para evitar replay de miles de eventos al arrancar:

- Cada 100 eventos o 24h, se escribe un snapshot del estado actual del registro
- Al iniciar: cargar último snapshot → aplicar solo eventos posteriores
- Los snapshots viejos se pueden compactar (guardar 1 cada 1000 eventos)

### 4.2 Índices secundarios

redb es key-value, no soporta índices secundarios nativos. Solución: **índices materializados en entries separadas.**

```
operational/
  _indexes/
    customers_by_name/
      "juan-pérez"          ← valor: [customer_id]
      "maría-garcía"        ← valor: [customer_id, customer_id]
    invoices_by_customer/
      "cust-123"            ← valor: [invoice_id, invoice_id, ...]
      "cust-456"            ← valor: [invoice_id]
    invoices_by_status/
      "open"                ← valor: [invoice_id, ...]
      "paid"                ← valor: [invoice_id, ...]
    products_by_category/
      "electronics"         ← valor: [product_id, ...]
```

**Actualización de índices:** el adapter de TanStack DB escribe los índices al mismo tiempo que el evento. Si un evento crea una factura con `customer_id: "cust-123"` y `status: "open"`, el adapter escribe:

```
1. Evento:    invoices/INV-1042/events/{hlc} → { type: "created", payload: {...} }
2. Índice:    _indexes/invoices_by_customer/cust-123 → append "INV-1042"
3. Índice:    _indexes/invoices_by_status/open → append "INV-1042"
```

**Los índices se syncan P2P** como cualquier otra entry de iroh-docs.

### 4.3 Particionamiento por sucursal

Cuando hay 100+ sucursales escribiendo a `operational`, el set reconciliation de iroh-docs puede volverse pesado. Solución:

```
operational/
  branch_001/
    invoices/...
    orders/...
  branch_002/
    invoices/...
    orders/...
  _global/
    customers/...     ← catálogos globales (synced a todos)
    products/...
```

Cada sucursal tiene su propio prefijo dentro de `operational`. Los catálogos globales están en `_global`. El adapter puede leer de `branch_*` o de `_global` según permisos.

### 4.4 Archivo histórico (Fase 3 con Iggy)

Las entradas de iroh-docs se streamean a Apache Iggy para backup, analytics y auditoría offline. Los datos en el dispositivo se pueden compactar/purgar sabiendo que Iggy tiene el archivo completo.

---

## 5. Sistema de permisos completo

### 5.1 Matriz de permisos por rol

```
                     control  catalogs  operational  payroll  reports  inventory  ...
admin                CRUD     CRUD      CRUD          CRUD     CRUD      CRUD
gerente_ventas       R        R         CRUD          —        R         R
vendedor             R        R         CRUD(own)     —        —         R
contador             R        R         R             R        CRUD      R
almacenista          R        R         R(inventory)  —        —         CRUD
rh                   R        R         —             CRUD     —         —
auditor              R        R         R             R        R         R
cliente (portal)     —        R(own)    R(own)        —        —         —
```

- **C** = Create
- **R** = Read
- **U** = Update
- **D** = Delete
- **own** = solo registros propios (ej: vendedor solo ve/edita SUS facturas)
- **—** = sin acceso

### 5.2 Permisos a nivel de campo

No todos los roles ven todos los campos de una entidad:

```typescript
fields: [
  {
    key: "cost", label: "Costo", type: "currency",
    permissions: { view: ["admin", "contador", "gerente_ventas"], edit: ["admin", "contador"] }
    // vendedor NO ve el campo "costo" en productos
  },
  {
    key: "salary", label: "Salario", type: "currency",
    permissions: { view: ["admin", "rh"], edit: ["admin"] }
    // solo admin y rh ven el salario en empleados
  },
]
```

### 5.3 Permisos a nivel de registro (Row-Level Security)

```typescript
// Vendedor solo ve sus propias facturas
permissions: {
  canView: (row, role, userId) => role === "admin" || row.author === userId,
}
```

---

## 6. Offline con resolución de conflictos

### 6.1 Estrategia de conflictos

Como cada empleado escribe a su propio key prefix (`invoices/<node_id>/...`), no hay conflictos de escritura. El único caso de conflicto es **edición concurrente del mismo registro por dos dispositivos de la misma persona** (ej: laptop y celular de Bob, ambos offline, ambos editan la misma factura).

**Resolución: Last-Write-Wins (LWW) por HLC timestamp.**

Cada entry tiene un HLC con `(ts, count, node)`. La entrada con mayor HLC gana. Si mismo timestamp, mayor count. Si mismo count, comparación lexicográfica de node_id.

### 6.2 Cola offline

Cuando el dispositivo está offline:
1. Mutaciones se aplican optimistamente en TanStack DB (UI instantánea)
2. Se encolan para `commit_event` cuando vuelva la conexión
3. Al reconectar: flush de cola → iroh-docs sync → reconciliación de estado

```
offlineQueue: [
  { entity: "invoices", id: "INV-1042", operation: "update", changes: { status: "paid" }, hlc: {...} },
  { entity: "invoices", id: "INV-1043", operation: "create", payload: {...}, hlc: {...} },
]
```

### 6.3 Conflictos de secuencia (números de factura)

Gemini lo preguntó: ¿cómo evitar que dos vendedores offline generen `INV-1001` al mismo tiempo?

**Solución: prefijo de sucursal + contador local.**

```
Formato: {branch_prefix}-{counter}-{node_suffix}
Ejemplo:  A-0042-XA1   (sucursal A, factura 42, dispositivo XA1)

Ventajas:
  - Sin colisiones (cada dispositivo tiene su propio contador)
  - Sin servidor central
  - Ordenable (por branch, luego counter)
  - La UI puede mostrar solo el counter (42) con el prefijo como metadata
```

---

## 7. Reporting y BI

### 7.1 Dashboards configurables

```typescript
interface DashboardDefinition {
  id: string;
  label: string;
  widgets: DashboardWidget[];
  refreshInterval?: number;  // segundos (default: live via useLiveQuery)
}

type DashboardWidget =
  | { type: "kpi"; title: string; query: LiveQuery; format: "number" | "currency" | "percent" }
  | { type: "chart"; title: string; chartType: "bar" | "line" | "pie" | "area"; query: LiveQuery; xField: string; yField: string }
  | { type: "table"; title: string; entity: string; view: string; maxRows: number }
  | { type: "list"; title: string; entity: string; filter: FilterRule[]; maxRows: number };
```

Los dashboards se alimentan de TanStack DB `useLiveQuery` — son reactivos y offline.

### 7.2 Reportes

Reportes predefinidos + reportes personalizados con un builder visual:

```
Report builder:
  1. Seleccionar entidad base (invoices)
  2. Agregar joins (customers, products)
  3. Agregar filtros (date range, status, branch)
  4. Agregar agrupaciones (by month, by salesperson, by category)
  5. Agregar métricas (SUM amount, COUNT, AVG)
  6. Elegir visualización (table, chart, pivot)
  7. Guardar / Exportar (PDF, Excel, CSV)
```

---

## 8. Plugin system

Para que terceros puedan extender Syntrix sin tocar el core:

```
plugins/
├── sat-mx/               ← facturación electrónica México
│   ├── fields/            ← rfc.ts, cfdi-uuid.ts, regimen-fiscal.ts
│   ├── entities/          ← cfdiEntity (facturas electrónicas)
│   ├── actions/           ← timbrarCFDI, cancelarCFDI
│   └── workflows/         ← workflow de timbrado
├── banking/               ← conciliación bancaria
├── ecommerce/             ← integración con tiendas online
└── ...
```

```typescript
interface SyntrixPlugin {
  id: string;
  name: string;
  version: string;
  fieldTypes?: FieldTypePlugin[];
  entities?: EntityDefinition[];
  dashboards?: DashboardDefinition[];
  reports?: ReportDefinition[];
  routes?: RouteDefinition[];
  hooks?: {
    onStartup?: () => void;
    onEntityCreate?: (entity: string, row: any) => void;
    onEntityUpdate?: (entity: string, id: string, changes: any) => void;
    onEntityDelete?: (entity: string, id: string) => void;
  };
}
```

---

## 9. Internacionalización (i18n)

```typescript
// Todas las strings de UI pasan por i18n
const { t } = useTranslation();

entity: {
  label: t("entities.customers.label"),        // "Clientes" / "Customers"
  labelPlural: t("entities.customers.plural"), // "Clientes" / "Customers"
}

// Traducciones por módulo
locales/
├── es/
│   ├── common.json
│   ├── entities.json
│   ├── fields.json
│   └── errors.json
└── en/
    ├── common.json
    ├── entities.json
    ├── fields.json
    └── errors.json
```

---

## 10. Performance targets

| Métrica | Target |
|---------|--------|
| Grid scroll FPS | 60 FPS con 100,000 filas |
| Apertura detail panel | < 50ms (instantáneo, datos locales) |
| Búsqueda global | < 100ms (índice en memoria) |
| Commit event | < 10ms (escritura local redb) |
| Sync P2P (2 peers, LAN) | < 500ms para entrada nueva |
| Startup (org con 10,000 registros) | < 2s (snapshots + differential sync) |
| Tamaño de app (Tauri) | < 50MB |
| Memoria (cliente web) | < 200MB con 100,000 registros |

---

## 11. Stack de la matriz

```
┌─ Frontend ────────────────────────────────────────┐
│  React 19 + TypeScript                             │
│  TanStack Router (type-safe routing)               │
│  TanStack DB (queries reactivas con live queries)  │
│  TanStack Query (data fetching para APIs externas) │
│  TanStack Table V9 (tablas pequeñas en detail)     │
│  TanStack Table + shadcn (grid principal, DOM-based)      │
│  shadcn/ui (design system base)                   │
│  Zod (schemas + validación)                        │
│  i18next (internacionalización)                    │
│  Recharts (gráficos en dashboards)                │
├───────────────────────────────────────────────────┤
│  Adapter (TypeScript, ~500 LOC)                    │
│  iroh-docs ↔ TanStack DB bridge                   │
│  - sync(): carga entries → collections             │
│  - subscribe(): eventos en tiempo real             │
│  - commit(): optimistic + rollback                 │
│  - indexes(): mantiene índices secundarios         │
│  - snapshots(): compacta historial                 │
├───────────────────────────────────────────────────┤
│  Backend (Rust, Tauri)                             │
│  iroh-docs (P2P sync + redb storage)               │
│  syntrix-docs (autorización, capabilities)         │
│  iroh-streams server (HTTP API para web clients)   │
│  (futuro) Apache Iggy (backup + analytics)         │
└───────────────────────────────────────────────────┘
```

---

## Resumen: lo que la matriz completa cubre y el MVP no

| Feature | MVP | Producto final |
|---------|-----|---------------|
| Entidades | 4 hardcodeadas | **100+ definidas por metadata** |
| Field types | 5 (text, number, date, relation, status) | **30+ con sistema de plugins** |
| Vistas | Solo grid | **Grid, kanban, calendar, gallery, gantt, map** |
| Permisos | 4 roles fijos | **Matriz de permisos granular (entidad + campo + registro)** |
| Workflows | No | **Motor de estados + transiciones + side effects** |
| Reporting | No | **Dashboards + report builder visual** |
| Búsqueda | No | **Búsqueda global tipo Spotlight (Ctrl+K)** |
| i18n | No | **Multi-idioma con lazy loading** |
| Offline | Básico | **Cola offline + HLC + conflict resolution** |
| Índices | No | **Índices secundarios materializados en redb** |
| Plugins | No | **Sistema de plugins para módulos de terceros** |
| Snapshots | No | **Snapshots periódicos para startup rápido** |
| Web client | Tauri solamente | **Web client vía iroh-streams HTTP** |
| Mobile | No | **Android/iOS vía Tauri mobile** |
| Escape hatches | No | **Entidades pueden reemplazar grid/detail con UI custom** |