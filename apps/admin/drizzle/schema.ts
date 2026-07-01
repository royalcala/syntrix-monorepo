import { sqliteTable, text, integer, index } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

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
