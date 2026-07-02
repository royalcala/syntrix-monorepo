/**
 * Column metadata for entity tables, hand-maintained alongside `schema.ts` (see
 * .kilo/plans/1782949593655-relational-cdc-migration.md, Fase 1 punto 7).
 *
 * This is the single source that `scripts/export-schema-json.mjs` reads to produce
 * `schema.json`, which Rust (`syntrix-core`/`SqlEngine`) loads at startup to build
 * typed INSERT/SELECT projections and FTS indexes generically, without hardcoding
 * column lists per entity in Rust.
 *
 * Keep in sync with `schema.ts` by hand: column `name` must match the physical SQLite
 * column name (snake_case) declared there.
 */

/** @typedef {{ name: string, type: "text" | "integer" | "real", nullable?: boolean, searchable?: boolean }} ColumnMeta */
/** @typedef {{ table: string, parentKey: string, columns: ColumnMeta[] }} ChildTableMeta */
/** @typedef {{ table: string, columns: ColumnMeta[], children?: ChildTableMeta[] }} EntityMeta */

/** @type {Record<string, EntityMeta>} */
export const entitySchemas = {
  customers: {
    table: "customers",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "name", type: "text", searchable: true },
      { name: "tax_id", type: "text", nullable: true, searchable: true },
      { name: "address", type: "text", nullable: true },
      { name: "phone", type: "text", nullable: true },
      { name: "email", type: "text", nullable: true, searchable: true },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
  },
  suppliers: {
    table: "suppliers",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "name", type: "text", searchable: true },
      { name: "tax_id", type: "text", nullable: true, searchable: true },
      { name: "address", type: "text", nullable: true },
      { name: "phone", type: "text", nullable: true },
      { name: "email", type: "text", nullable: true, searchable: true },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
  },
  products: {
    table: "products",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "name", type: "text", searchable: true },
      { name: "sku", type: "text", nullable: true, searchable: true },
      { name: "price", type: "real" },
      { name: "unit", type: "text" },
      { name: "category", type: "text", searchable: true },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
  },
  invoices: {
    table: "invoices",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "customer_id", type: "text", searchable: true },
      { name: "amount", type: "real" },
      { name: "status", type: "text", searchable: true },
      { name: "tax_rate", type: "real" },
      { name: "date", type: "text" },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
    children: [
      {
        table: "invoice_items",
        parentKey: "invoice_id",
        columns: [
          { name: "org_id", type: "text" },
          { name: "invoice_id", type: "text" },
          { name: "line_id", type: "text" },
          { name: "product_id", type: "text" },
          { name: "qty", type: "real" },
          { name: "price", type: "real" },
          { name: "change_time", type: "integer" },
          { name: "node_id", type: "text" },
        ],
      },
    ],
  },
  orders: {
    table: "orders",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "customer_id", type: "text", searchable: true },
      { name: "status", type: "text", searchable: true },
      { name: "date", type: "text" },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
    children: [
      {
        table: "order_items",
        parentKey: "order_id",
        columns: [
          { name: "org_id", type: "text" },
          { name: "order_id", type: "text" },
          { name: "line_id", type: "text" },
          { name: "product_id", type: "text" },
          { name: "qty", type: "real" },
          { name: "change_time", type: "integer" },
          { name: "node_id", type: "text" },
        ],
      },
    ],
  },
  payroll: {
    table: "payroll",
    columns: [
      { name: "org_id", type: "text" },
      { name: "doc_id", type: "text" },
      { name: "employee_id", type: "text", searchable: true },
      { name: "employee_name", type: "text", searchable: true },
      { name: "period", type: "text", searchable: true },
      { name: "gross_amount", type: "real" },
      { name: "deductions", type: "real" },
      { name: "net_amount", type: "real" },
      { name: "status", type: "text", searchable: true },
      { name: "change_time", type: "integer" },
      { name: "node_id", type: "text" },
    ],
  },
};
