import { sqliteTable, text, integer, real, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

function syncMeta() {
  return {
    changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
    nodeId: text("node_id").notNull().default(""),
  };
}

// ---------------------------------------------------------------------------
// Customers
// ---------------------------------------------------------------------------
export const customers = sqliteTable("customers", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  taxId: text("tax_id"),
  address: text("address"),
  phone: text("phone"),
  email: text("email"),
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_customers_org").on(table.orgId),
]);

// ---------------------------------------------------------------------------
// Suppliers
// ---------------------------------------------------------------------------
export const suppliers = sqliteTable("suppliers", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  taxId: text("tax_id"),
  address: text("address"),
  phone: text("phone"),
  email: text("email"),
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_suppliers_org").on(table.orgId),
]);

// ---------------------------------------------------------------------------
// Products
// ---------------------------------------------------------------------------
export const products = sqliteTable("products", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  name: text("name").notNull(),
  sku: text("sku"),
  price: real("price").notNull(),
  unit: text("unit").notNull().default("pza"),
  category: text("category").notNull().default("general"),
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_products_org").on(table.orgId),
]);

// ---------------------------------------------------------------------------
// Invoices
// ---------------------------------------------------------------------------
export const invoices = sqliteTable("invoices", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  customerId: text("customer_id").notNull(),
  amount: real("amount").notNull(),
  status: text("status").notNull().default("draft"),
  taxRate: real("tax_rate").notNull().default(0.16),
  date: text("date").notNull(),
  ...syncMeta(),
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
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.invoiceId, table.lineId] }),
  index("idx_invoice_items_invoice").on(table.orgId, table.invoiceId),
]);

// ---------------------------------------------------------------------------
// Orders
// ---------------------------------------------------------------------------
export const orders = sqliteTable("orders", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  customerId: text("customer_id").notNull(),
  status: text("status").notNull().default("pending"),
  date: text("date").notNull(),
  ...syncMeta(),
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
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.orderId, table.lineId] }),
  index("idx_order_items_order").on(table.orgId, table.orderId),
]);

// ---------------------------------------------------------------------------
// Payroll
// ---------------------------------------------------------------------------
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
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_payroll_org").on(table.orgId),
]);

// ---------------------------------------------------------------------------
// View Definitions (AI-generated views, synced via CDC)
// ---------------------------------------------------------------------------
export const viewDefinitions = sqliteTable("view_definitions", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  sqlText: text("sql").notNull(),
  entityName: text("entity").notNull(),
  componentsJson: text("components_json").notNull(),
  rootComponent: text("root").notNull(),
  metaJson: text("meta_json").notNull(),
  createdBy: text("created_by").notNull(),
  tags: text("tags"),
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_view_definitions_org").on(table.orgId),
]);

// ---------------------------------------------------------------------------
// IA Queries (CDC fallback queue when IA is offline)
// ---------------------------------------------------------------------------
export const iaQueries = sqliteTable("ia_queries", {
  orgId: text("org_id").notNull(),
  docId: text("doc_id").notNull(),
  queryText: text("text").notNull(),
  status: text("status").notNull().default("pending"),
  viewId: text("view_id"),
  ...syncMeta(),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.docId] }),
  index("idx_ia_queries_org").on(table.orgId),
  index("idx_ia_queries_status").on(table.orgId, table.status),
]);
