import { sqliteTable, text, integer, real, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

// Admin replicates the same relational entity tables as the client (Decision 6: "Admin =
// réplica relacional completa"), populated from CDC events received over gossip (Fase 4)
// instead of from local writes. Column shapes are kept identical to
// apps/client/drizzle/schema.ts / schema-meta.mjs so the same typed-projection code
// (schema.json-driven) can be reused to apply CDC on both sides.

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

// Data-audit trail (Decision 5): one row per CDC change received from any peer.
// `event_log` in the admin app is no longer a sync transport, only the durable audit
// history: `entity` / `change_type` (insert|update|delete) / `doc_id` / row image (JSON
// reconstructed from CDC columns) / `change_time` / `node_id`. `AuditTrail.tsx` becomes a
// saved view over this table (Fase 4, tarea 23).
export const eventLog = sqliteTable("event_log", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  orgId: text("org_id").notNull(),
  entity: text("entity").notNull(),
  changeType: text("change_type").notNull(),
  docId: text("doc_id").notNull(),
  rowImage: text("row_image").notNull().default("{}"),
  changeTime: integer("change_time").notNull(),
  nodeId: text("node_id").notNull().default(""),
  createdAt: integer("created_at").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  index("idx_event_log_org").on(table.orgId, table.changeTime),
  index("idx_event_log_entity").on(table.orgId, table.entity, table.docId),
]);

// Saved SQL console queries (Fase 4, tarea 22). `AuditTrail`/`Logs` ship as seeded rows here.
export const savedViews = sqliteTable("saved_views", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  sqlQuery: text("sql_query").notNull(),
  createdAt: integer("created_at").notNull().default(sql`(unixepoch('now') * 1000)`),
  updatedAt: integer("updated_at").notNull().default(sql`(unixepoch('now') * 1000)`),
});
