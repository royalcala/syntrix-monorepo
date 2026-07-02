import { SqlConsole } from "./SqlConsole";

/**
 * Audit Trail is now a thin wrapper around the generic read-only SQL console (Fase 4, tarea
 * 23): the bespoke `audit_query` filtering UI/state has been replaced by a plain SELECT over
 * `event_log`, matching the "Audit Trail" saved view seeded at startup
 * (sql_console.rs::seed_default_saved_views). `org` scopes the query but any user can still
 * edit it or run their own from the same console.
 *
 * `run_sql` only accepts a single opaque query string (no bound parameters from the
 * frontend), so `org` is escaped before being interpolated into the WHERE clause to avoid
 * breaking or extending the query if an org name ever contains a single quote.
 */
function escapeSqlStringLiteral(value: string): string {
  return value.replace(/'/g, "''");
}

export function AuditTrail({ org }: { org: string }) {
  const query = org
    ? `SELECT change_time, entity, change_type, doc_id, node_id, row_image FROM event_log WHERE org_id = '${escapeSqlStringLiteral(org)}' ORDER BY change_time DESC`
    : "SELECT change_time, entity, change_type, doc_id, node_id, row_image FROM event_log ORDER BY change_time DESC";

  return <SqlConsole initialQuery={query} />;
}
