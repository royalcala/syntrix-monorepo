import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type AuditEntry = {
  key: string;
  event_type: string;
  hlc_ts: number;
  schema_version: number;
  entity: string;
  doc_id: string;
  payload: Record<string, unknown>;
};

export function AuditTrail({ org }: { org: string }) {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [filterEntity, setFilterEntity] = useState("");
  const [filterEventType, setFilterEventType] = useState("");
  const [expandedKey, setExpandedKey] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!org) return;
    setLoading(true);
    try {
      const result: AuditEntry[] = await invoke("audit_query", {
        orgId: org,
        filter: {
          entity: filterEntity || null,
          event_type: filterEventType || null,
          node: null,
          doc_id: null,
          since_ts: null,
          until_ts: null,
        },
        limit: 100,
        offset: 0,
      });
      setEntries(result);
    } catch (e) {
      console.error("audit_query failed:", e);
    }
    setLoading(false);
  }, [org, filterEntity, filterEventType]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="h-full flex flex-col">
      {/* Filters */}
      <div className="flex items-center gap-3 pb-4 shrink-0">
        <input
          className="h-9 rounded-md border px-3 py-1.5 text-sm bg-background w-40"
          placeholder="Entidad (ej. customers)"
          value={filterEntity}
          onChange={(e) => setFilterEntity(e.target.value)}
        />
        <input
          className="h-9 rounded-md border px-3 py-1.5 text-sm bg-background w-44"
          placeholder="Tipo (ej. customer.created)"
          value={filterEventType}
          onChange={(e) => setFilterEventType(e.target.value)}
        />
        <button
          className="h-9 px-4 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90"
          onClick={load}
        >
          {loading ? "Cargando..." : "Filtrar"}
        </button>
        <span className="text-xs text-muted-foreground ml-auto">
          {entries.length} eventos
        </span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-y-auto border rounded-lg">
        <table className="w-full text-sm">
          <thead className="bg-muted/50 sticky top-0">
            <tr>
              <th className="text-left px-3 py-2 font-medium">Timestamp</th>
              <th className="text-left px-3 py-2 font-medium">Tipo</th>
              <th className="text-left px-3 py-2 font-medium">Entidad</th>
              <th className="text-left px-3 py-2 font-medium">Doc ID</th>
              <th className="text-center px-3 py-2 font-medium">v</th>
              <th className="text-left px-3 py-2 font-medium">Payload</th>
            </tr>
          </thead>
          <tbody>
            {entries.length === 0 ? (
              <tr>
                <td colSpan={6} className="text-center py-8 text-muted-foreground text-sm">
                  No se encontraron eventos
                </td>
              </tr>
            ) : (
              entries.map((e) => (
                <tr key={e.key} className="border-t hover:bg-muted/30">
                  <td className="px-3 py-2 font-mono text-xs text-muted-foreground whitespace-nowrap">
                    {new Date(e.hlc_ts / 1000).toLocaleString()}
                  </td>
                  <td className="px-3 py-2 font-mono text-xs">{e.event_type}</td>
                  <td className="px-3 py-2 text-xs capitalize">{e.entity}</td>
                  <td className="px-3 py-2 font-mono text-xs text-muted-foreground">{e.doc_id.slice(0, 20)}…</td>
                  <td className="px-3 py-2 text-center text-xs">{e.schema_version}</td>
                  <td className="px-3 py-2">
                    <button
                      className="text-xs text-blue-500 hover:underline"
                      onClick={() => setExpandedKey(expandedKey === e.key ? null : e.key)}
                    >
                      {expandedKey === e.key ? "Ocultar" : "Ver JSON"}
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Expanded JSON detail */}
      {expandedKey && (
        <div className="shrink-0 border-t bg-muted/20 p-4 overflow-auto max-h-64">
          <pre className="text-xs font-mono whitespace-pre-wrap">
            {JSON.stringify(entries.find((e) => e.key === expandedKey)?.payload ?? {}, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
