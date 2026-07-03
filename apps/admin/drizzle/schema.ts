// Re-export shared entity tables so drizzle-kit discovers them
export {
  customers, suppliers, products, invoices, invoiceItems,
  orders, orderItems, payroll,
} from "../../../packages/shared-drizzle/src/entities";

import { sqliteTable, integer, text, index } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

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
