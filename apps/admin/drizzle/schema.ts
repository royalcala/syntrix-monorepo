// Re-export shared entity tables so drizzle-kit discovers them
export {
  customers, suppliers, products, invoices, invoiceItems,
  orders, orderItems, payroll,
} from "../../../packages/shared-drizzle/src/entities";

import { sqliteTable, integer, text, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

// Admin-specific: organizations
export const adminOrgs = sqliteTable("admin_orgs", {
  name: text("name").primaryKey(),
  topicId: text("topic_id").notNull(),
  createdAt: integer("created_at").notNull().default(sql`(unixepoch('now') * 1000)`),
});

// Admin-specific: devices managed by this admin node
export const adminDevices = sqliteTable("admin_devices", {
  orgId: text("org_id").notNull(),
  nodeId: text("node_id").notNull(),
  active: integer("active", { mode: "boolean" }).notNull().default(true),
  role: text("role").notNull().default("sales"),
  person: text("person").notNull().default(""),
  name: text("name").notNull().default(""),
  deviceAddr: text("device_addr").notNull().default(""),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.nodeId] }),
  index("idx_admin_devices_org").on(table.orgId),
]);

// Admin-specific: roles defined by this admin
export const adminRoles = sqliteTable("admin_roles", {
  orgId: text("org_id").notNull(),
  roleName: text("role_name").notNull(),
  canOpen: text("can_open").notNull().default("[]"),
  canWrite: text("can_write").notNull().default("[]"),
  changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.roleName] }),
  index("idx_admin_roles_org").on(table.orgId),
]);

// Admin-specific: audit trail populated from CDC events (Fase 4)
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

// Admin-specific: saved SQL console queries
export const savedViews = sqliteTable("saved_views", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  sqlQuery: text("sql_query").notNull(),
  createdAt: integer("created_at").notNull().default(sql`(unixepoch('now') * 1000)`),
  updatedAt: integer("updated_at").notNull().default(sql`(unixepoch('now') * 1000)`),
});
