---
title: "Visión a Escala — La Nueva Matriz Soberana"
description: "El destino del producto. No es el estado actual — es la matriz completa que emerge cuando los nodos libres se sincronizan sin intermediarios."
---

# Visión a Escala

Syntrix no es un ERP. Es un **Espacio Operativo Autónomo**: una plataforma de colaboración soberana donde cada colectivo — empresa, familia, equipo — tiene control absoluto sobre su información y su operación.

Este documento describe el destino del producto. No es el estado actual — es la matriz completa que emerge cuando los nodos libres se sincronizan sin intermediarios.

Los 4 pilares que guían cada decisión:

1. **Soberanía Absoluta** — tus datos viven en tus dispositivos. Nadie más tiene acceso.
2. **Inmediatez Radical** — sin viajes a servidores. Cada clic ocurre en cero milisegundos.
3. **Sincronía Orgánica** — los nodos se entrelazan en silencio, sin que el usuario lo note.
4. **Resiliencia por Diseño** — sin internet, el espacio sigue operando. La dependencia se terminó.

---

## 1. Escala de la matriz

| Dimensión | Estado actual | Matriz completa |
|-----------|--------------|-----------------|
| Espacios operativos | 6 (clientes, proveedores, productos, facturas, órdenes, nómina) | **100+** (inventario, compras, contabilidad, proyectos, CRM...) |
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
│   ├── ui/                ← shadcn primitives (Button, Input, Select, Dialog...)
│   ├── data/              ← componentes de datos (EntityGrid, DetailPanel, Kanban...)
│   ├── layout/            ← AppShell, Sidebar, Toolbar
│   └── feedback/          ← Toast, Dialog, EmptyState, ErrorBoundary
└── patterns/
    ├── entity-page.tsx     ← patrón estándar para cualquier entidad
    ├── form-pattern.tsx    ← patrón de formularios (create/edit)
    └── list-detail.tsx     ← patrón maestro-detalle
```

### 3.2 FieldRegistry extendido (100+ field types)

El FieldRegistry crece con el producto. Cada field type es un módulo independiente. Hoy hay 8 base (text, number, currency, date, select, status, relation, boolean). La visión es llegar a 30+ con sistema de plugins:

```
fields/
├── registry.ts             ← registro central
├── types/
│   ├── text.ts             ← texto simple
│   ├── number.ts           ← numérico
│   ├── currency.ts         ← moneda con símbolo + formato
│   ├── date.ts             ← fecha con datepicker
│   ├── select.ts           ← dropdown (opciones fijas)
│   ├── relation.ts         ← relación a otra entidad
│   ├── status.ts           ← badge de estado con color
│   ├── boolean.ts          ← toggle / checkbox
│   ├── multiselect.ts      ← selección múltiple
│   ├── computed.ts         ← campo calculado (fórmula)
│   ├── email.ts            ← email con validación
│   ├── phone.ts            ← teléfono con formato
│   ├── image.ts            ← imagen (thumbnail + lightbox)
│   ├── file.ts             ← archivo adjunto (P2P transfer)
│   ├── richtext.ts         ← texto enriquecido
│   ├── tags.ts             ← etiquetas múltiples
│   ├── lookup.ts           ← búsqueda en otra entidad (autocomplete)
│   ├── rollup.ts           ← agregación de relación (SUM, COUNT, AVG)
│   └── ...
└── plugins/                ← field types de terceros
    ├── sat/
    │   ├── rfc.ts          ← validación RFC México
    │   ├── cfdi-uuid.ts    ← UUID de factura electrónica
    │   └── regimen-fiscal.ts
    └── ...
```

### 3.3 EntityDefinition completo

La entidad es el concepto unificador. Hoy define campos, vistas, y tabs de detalle. La visión extiende a workflows, permisos por campo, y escape hatches:

```typescript
interface EntityDefinition {
  id: string;
  label: string;
  icon: string;
  fields: FieldConfig[];
  views: ViewDefinition[];
  detail: { tabs: DetailTab[] };
  // Futuro:
  workflow?: WorkflowDefinition;      // estados + transiciones + side effects
  permissions?: {
    canView: PermissionCheck;          // Row-Level Security
    canCreate: PermissionCheck;
    canEdit: PermissionCheck;
    canDelete: PermissionCheck;
  };
  override?: {
    grid?: React.ComponentType;       // grid personalizado (POS, calendario)
    fullPage?: React.ComponentType;   // página completa (dashboard, config)
  };
}
```

### 3.4 Vistas del workspace (más allá del grid)

No todo es grid+detail. El workspace soporta múltiples layouts:

```
Layouts disponibles:
├── grid             ← tabla (TanStack Table + shadcn) — 80% de las entidades
├── kanban           ← tablero kanban (oportunidades, proyectos)
├── calendar         ← calendario (entregas, citas)
├── gallery          ← galería de cards (productos con imágenes)
├── dashboard        ← dashboards con widgets
└── custom           ← componente React arbitrario (escape hatch)
```

### 3.5 Búsqueda global (tipo Spotlight/Linear)

Ya implementada con **Limbo FTS** (Rust embebido):

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
  │                                     │
  │ Acciones                            │
  │   + Nueva factura                   │
  │   Ir a → Facturas                   │
  └─────────────────────────────────────┘
```

Motor: Limbo FTS con fuzzy search, BM25 ranking, snippets con highlights. Se alimenta automáticamente desde el indexador FTS.

---

## 4. Arquitectura de datos a escala

### 4.1 Estructura de eventos en P2P

```
evt:{hlc_ts}:{hlc_count}:{node_id}
  → valor: { type, hlc, schema_version, payload }
```

El log es append-only e inmutable. La proyección a Limbo SQL (estado actual consolidado) es volátil y reconstruible.

**Snapshots periódicos** (visión futura) para evitar replay de miles de eventos al arrancar:
- Cada 100 eventos o 24h, se escribe un snapshot del estado actual del registro
- Al iniciar: cargar último snapshot → aplicar solo eventos posteriores
- Los snapshots viejos se pueden compactar (guardar 1 cada 1000 eventos)

### 4.2 Índices secundarios (ya implementado)

Los índices se definen en esquemas Drizzle y se traducen a sentencias `CREATE INDEX` sobre Limbo SQL. Las columnas con `@index()` en el schema Drizzle generan índices simples; los índices compuestos se declaran explícitamente en el schema.

Los índices son **locales y derivados**, nunca se sincronizan P2P. Cada peer reconstruye sus índices del event log.

### 4.3 Particionamiento por sucursal (visión futura)

Cuando hay 100+ sucursales escribiendo al mismo namespace, el set reconciliation P2P puede volverse pesado.

Con namespaces por entidad, el aislamiento es granular y la contención se minimiza.

La UI de administración usa una **matriz de permisos** plana por entidad, sin agrupación por namespace.

### 5.2 Permisos a nivel de campo (visión futura)

No todos los roles ven todos los campos de una entidad:

```typescript
fields: [
  {
    key: "cost", label: "Costo", type: "currency",
    permissions: { view: ["admin", "contador"], edit: ["admin"] }
    // vendedor NO ve el campo "costo" en productos
  },
  {
    key: "salary", label: "Salario", type: "currency",
    permissions: { view: ["admin", "rh"], edit: ["admin"] }
  },
]
```

### 5.3 Permisos a nivel de registro (visión futura)

```typescript
// Vendedor solo ve sus propias facturas
permissions: {
  canView: (row, role, userId) => role === "admin" || row.author === userId,
}
```

---

## 6. Offline con resolución de conflictos

### 6.1 Estrategia de conflictos

Cada evento tiene un HLC con `(ts, count, node)`. La entrada con mayor HLC gana (Last-Write-Wins). Si mismo timestamp, mayor count. Si mismo count, comparación lexicográfica de node_id.

### 6.2 Folios sin colisión (visión, no implementado)

Para evitar que dos vendedores offline generen `INV-1001` al mismo tiempo:

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

## 7. Plugin system (Fase 3)

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
  hooks?: {
    onEntityCreate?: (entity: string, row: any) => void;
    onEntityUpdate?: (entity: string, id: string, changes: any) => void;
  };
}
```

---

## 8. Performance targets

| Métrica | Target | Estado actual |
|---------|--------|---------------|
| Grid scroll FPS | 60 FPS con 100,000 filas | ⚠️ Falta virtualización |
| Apertura detail panel | < 50ms | ✅ Instantáneo |
| Búsqueda global | < 100ms | ✅ Limbo FTS |
| Commit event | < 10ms | ✅ Limbo SQL |
| Sync P2P (2 peers, LAN) | < 500ms | ✅ |
| Startup (org con 10,000 registros) | < 2s | ⚠️ Falta snapshots |
| Tamaño de app (Tauri) | < 50MB | ✅ |
| Memoria | < 200MB con 100,000 registros | ✅ Solo página actual en RAM |

---

## 9. Stack tecnológico

```
┌─ Frontend ────────────────────────────────────────┐
│  React 19 + TypeScript                             │
│  TanStack Table V9 (grid, DOM-based, shadcn)       │
│  TanStack Query (data fetching + cache)            │
│  TanStack Form (validación de formularios)         │
│  shadcn/ui + Radix (design system base)            │
│  Zod (schemas + validación)                        │
│  lucide-react (iconos)                             │
├───────────────────────────────────────────────────┤
│  Tauri IPC (comunicación Rust ↔ React)             │
├───────────────────────────────────────────────────┤
│  Backend (Rust, Tauri)                             │
│  libp2p (P2P networking)                          │
│  turso_core (SQL + FTS + CDC)                     │
│  syntrix-core (P2P auth, sync, heartbeats)        │
│  (futuro) Almacenamiento descentralizado (Arweave) │
└───────────────────────────────────────────────────┘
```

---

## Resumen: lo que la matriz completa cubre y el estado actual no

| Feature | Estado actual | Producto final |
|---------|--------------|---------------|
| Entidades | 6 definidas en registry | **100+ definidas por metadata + plugins** |
| Field types | 8 base | **30+ con sistema de plugins** |
| Vistas | Solo grid | **Grid, kanban, calendar, gallery, dashboard** |
| Permisos | Entidad-level (can_open/can_write) | **Entidad + campo + registro** |
| Workflows | No | **Motor de estados + transiciones + side effects** |
| Reporting | Auditoría de eventos | **Dashboards + report builder visual** |
| Búsqueda | ✅ Limbo FTS (Ctrl+K) | ✅ (completo) |
| i18n | No | **Multi-idioma con lazy loading** |
| Offline | Básico (LWW por HLC) | **Cola offline + conflict resolution + snapshots** |
| Índices | ✅ Schema-driven (Drizzle + SQL) | ✅ (completo) |
| Plugins | No | **Sistema de plugins para módulos de terceros** |
| Web client | Tauri solamente | **Web client vía HTTP API** |
| Mobile | No | **Android/iOS vía Tauri mobile** |
| Escape hatches | No | **Entidades pueden reemplazar grid/detail con UI custom** |
