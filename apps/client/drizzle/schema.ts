import { sqliteTable, text, integer, real, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

// ---------------------------------------------------------------------------
// Sync metadata shared by every relational entity table (see
// .kilo/plans/1782949593655-relational-cdc-migration.md, decisiones 1 y 4).
// `org_id` + `doc_id` remain the business primary key (doc_id is the entity's own id,
// e.g. invoice id). `change_time` + `node_id` back the CDC-native LWW conflict
// resolution: `apply_cdc_events` keeps the row whose `change_time` is newer, and
// `node_id` identifies the author's node for permission checks and tie-breaking.
// ---------------------------------------------------------------------------

export const customers = sqliteTable("customers", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  taxId: text("tax_id"),
  address: text("address"),
  phone: text("phone"),
  email: text("email"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_customers_org").on(table.orgId),
]);

export const suppliers = sqliteTable("suppliers", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  taxId: text("tax_id"),
  address: text("address"),
  phone: text("phone"),
  email: text("email"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_suppliers_org").on(table.orgId),
]);

export const products = sqliteTable("products", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  sku: text("sku"),
  price: real("price").notNull(),
  unit: text("unit").notNull().default("pza"),
  category: text("category").notNull().default("general"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_products_org").on(table.orgId),
]);

export const invoices = sqliteTable("invoices", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  customerId: text("customer_id").notNull(),
  amount: real("amount").notNull(),
  status: text("status").notNull().default("draft"),
  taxRate: real("tax_rate").notNull().default(0.16),
  date: text("date").notNull(),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_invoices_org").on(table.orgId),
  index("idx_invoices_customer").on(table.orgId, table.customerId),
]);

// Line items: own row per item, FK -> invoices, synced individually via CDC (each
// row has its own change_time/node_id for LWW). Update semantics = delete+reinsert
// children on the client write path (see events.rs / SqlEngine, Fase 2).
export const invoiceItems = sqliteTable("invoice_items", {
  orgId: text("org_id").notNull(),
  invoiceId: text("invoice_id").notNull(),
  lineId: text("line_id").notNull(),
  productId: text("product_id").notNull(),
  qty: real("qty").notNull(),
  price: real("price").notNull(),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.invoiceId, table.lineId] }),
  index("idx_invoice_items_invoice").on(table.orgId, table.invoiceId),
]);

export const orders = sqliteTable("orders", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  customerId: text("customer_id").notNull(),
  status: text("status").notNull().default("pending"),
  date: text("date").notNull(),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_orders_org").on(table.orgId),
  index("idx_orders_customer").on(table.orgId, table.customerId),
]);

export const orderItems = sqliteTable("order_items", {
  orgId: text("org_id").notNull(),
  orderId: text("order_id").notNull(),
  lineId: text("line_id").notNull(),
  productId: text("product_id").notNull(),
  qty: real("qty").notNull(),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.orderId, table.lineId] }),
  index("idx_order_items_order").on(table.orgId, table.orderId),
]);

export const payroll = sqliteTable("payroll", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  employeeId: text("employee_id").notNull(),
  employeeName: text("employee_name").notNull(),
  period: text("period").notNull(),
  grossAmount: real("gross_amount").notNull(),
  deductions: real("deductions").notNull().default(0),
  netAmount: real("net_amount").notNull(),
  status: text("status").notNull().default("draft"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
  nodeId: text("node_id").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_payroll_org").on(table.orgId),
]);

export const members = sqliteTable("members", {
  orgId: text("org_id").notNull(),
  nodeId: text("node_id").notNull(),
  data: text("data").notNull().default("{}"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.nodeId] }),
  index("idx_members_org").on(table.orgId),
]);

export const roles = sqliteTable("roles", {
  orgId: text("org_id").notNull(),
  roleName: text("role_name").notNull(),
  data: text("data").notNull().default("{}"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.roleName] }),
  index("idx_roles_org").on(table.orgId),
]);

export const heartbeats = sqliteTable("heartbeats", {
  orgId: text("org_id").notNull(),
  nodeId: text("node_id").notNull(),
  ts: integer("ts").notNull().default(0),
  data: text("data").notNull().default("{}"),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.nodeId] }),
  index("idx_heartbeats_org").on(table.orgId, table.nodeId),
]);

// `event_log` is no longer the sync transport (see Decision 5 in the plan). It stays only
// as a client-local record of locally-committed business events for reference/debugging;
// the durable audit trail of record-level changes now lives in the admin DB, populated
// from CDC.
export const eventLog = sqliteTable("event_log", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  orgId: text("org_id").notNull(),
  key: text("key").notNull().unique(),
  eventType: text("event_type").notNull(),
  hlcTs: integer("hlc_ts").notNull(),
  hlcCount: integer("hlc_count").notNull(),
  hlcNode: text("hlc_node").notNull(),
  schemaVersion: integer("schema_version").notNull().default(1),
  entity: text("entity").notNull().default(""),
  payload: text("payload").notNull().default("{}"),
  createdAt: integer("created_at").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  index("idx_event_log_org").on(table.orgId, table.hlcTs),
]);

// Cursor bookkeeping for the CDC-native sync loop (Fase 3): last `turso_cdc.change_id`
// successfully published per org, so the loop can resume after a restart without
// re-scanning from the beginning.
export const cdcCursor = sqliteTable("cdc_cursor", {
  orgId: text("org_id").primaryKey(),
  lastChangeId: integer("last_change_id").notNull().default(0),
  updatedAt: integer("updated_at").notNull().default(sql`(unixepoch('now') * 1000)`),
});
