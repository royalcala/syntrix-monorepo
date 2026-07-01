import { sqliteTable, text, integer, index, primaryKey } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

function entityTable(name: string) {
  return sqliteTable(name, {
    orgId: text("org_id").notNull(),
    docId: text("doc_id").notNull(),
    payload: text("payload").notNull().default("{}"),
    ftsTitle: text("fts_title").notNull().default(""),
    ftsBody: text("fts_body").notNull().default(""),
    changeTime: integer("change_time").notNull().default(sql`(unixepoch('now') * 1000)`),
    nodeId: text("node_id").notNull().default(""),
  }, (table) => [
    primaryKey({ columns: [table.orgId, table.docId] }),
  ]);
}

export const customers = entityTable("customers");
export const suppliers = entityTable("suppliers");
export const products = entityTable("products");
export const invoices = entityTable("invoices");
export const orders = entityTable("orders");
export const payroll = entityTable("payroll");

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

export const hlcTracker = sqliteTable("hlc_tracker", {
  orgId: text("org_id").notNull(),
  entity: text("entity").notNull(),
  docId: text("doc_id").notNull(),
  hlcTs: integer("hlc_ts").notNull().default(0),
  hlcCount: integer("hlc_count").notNull().default(0),
  hlcNode: text("hlc_node").notNull().default(""),
}, (table) => [
  primaryKey({ columns: [table.orgId, table.entity, table.docId] }),
]);
