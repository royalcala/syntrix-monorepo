// Re-export shared entity tables so drizzle-kit discovers them
export {
  customers, suppliers, products, invoices, invoiceItems,
  orders, orderItems, payroll,
  viewDefinitions, iaQueries,
} from "../../../packages/shared-drizzle/src/entities";

import { sqliteTable, text, integer, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

// Client-specific: device/peer roster, role permissions, heartbeats, CDC cursor

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

export const cdcCursor = sqliteTable("cdc_cursor", {
  orgId: text("org_id").primaryKey(),
  lastChangeId: integer("last_change_id").notNull().default(0),
  updatedAt: integer("updated_at").notNull().default(sql`(unixepoch('now') * 1000)`),
});
