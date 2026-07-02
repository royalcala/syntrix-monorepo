import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type SqlResult = {
  columns: string[];
  rows: unknown[][];
  truncated: boolean;
};

type SavedView = {
  id: string;
  name: string;
  sql_query: string;
  created_at: number;
  updated_at: number;
};

const PAGE_SIZE = 100;

/**
 * Read-only SQL console (Fase 4, tarea 22): runs ad-hoc SELECT queries against the admin's
 * relational replica via the `run_sql` command (Rust rejects anything but SELECT/WITH), with
 * pagination and saved views. `AuditTrail`/`Logs` are now just saved views over `event_log`
 * (tarea 23) rather than bespoke screens.
 */
export function SqlConsole({ initialQuery }: { initialQuery?: string }) {
  const [query, setQuery] = useState(initialQuery ?? "SELECT * FROM event_log ORDER BY change_time DESC");
  const [result, setResult] = useState<SqlResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [page, setPage] = useState(0);
  const [savedViews, setSavedViews] = useState<SavedView[]>([]);
  const [newViewName, setNewViewName] = useState("");

  const loadSavedViews = useCallback(async () => {
    try {
      const views = await invoke<SavedView[]>("list_saved_views");
      setSavedViews(views);
    } catch (e) {
      console.error("list_saved_views failed:", e);
    }
  }, []);

  useEffect(() => {
    loadSavedViews();
  }, [loadSavedViews]);

  useEffect(() => {
    if (initialQuery) {
      setQuery(initialQuery);
    }
  }, [initialQuery]);

  const runQuery = useCallback(async (q: string, pageArg: number) => {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<SqlResult>("run_sql", {
        query: q,
        limit: PAGE_SIZE,
        offset: pageArg * PAGE_SIZE,
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
      setResult(null);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    runQuery(query, page);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleRun = () => {
    setPage(0);
    runQuery(query, 0);
  };

  const handleNextPage = () => {
    const next = page + 1;
    setPage(next);
    runQuery(query, next);
  };

  const handlePrevPage = () => {
    const prev = Math.max(0, page - 1);
    setPage(prev);
    runQuery(query, prev);
  };

  const handleSaveView = async () => {
    if (!newViewName.trim()) return;
    try {
      await invoke("create_saved_view", { name: newViewName.trim(), sqlQuery: query });
      setNewViewName("");
      loadSavedViews();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDeleteView = async (id: string) => {
    try {
      await invoke("delete_saved_view", { id });
      loadSavedViews();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleLoadView = (view: SavedView) => {
    setQuery(view.sql_query);
    setPage(0);
    runQuery(view.sql_query, 0);
  };

  return (
    <div className="h-full flex gap-4">
      {/* Saved views sidebar */}
      <div className="w-56 shrink-0 border rounded-lg overflow-y-auto flex flex-col">
        <div className="p-2 border-b text-xs font-medium text-muted-foreground">Vistas guardadas</div>
        <div className="flex-1 overflow-y-auto">
          {savedViews.map((v) => (
            <div key={v.id} className="flex items-center justify-between px-2 py-1.5 hover:bg-muted/50 text-sm group">
              <button className="text-left flex-1 truncate" onClick={() => handleLoadView(v)}>
                {v.name}
              </button>
              <button
                className="opacity-0 group-hover:opacity-100 text-xs text-muted-foreground hover:text-destructive px-1"
                onClick={() => handleDeleteView(v.id)}
              >
                ×
              </button>
            </div>
          ))}
        </div>
        <div className="p-2 border-t flex gap-1">
          <input
            className="flex-1 h-8 rounded border px-2 text-xs bg-background"
            placeholder="Nombre de vista"
            value={newViewName}
            onChange={(e) => setNewViewName(e.target.value)}
          />
          <button className="h-8 px-2 rounded bg-primary text-primary-foreground text-xs" onClick={handleSaveView}>
            Guardar
          </button>
        </div>
      </div>

      {/* Query editor + results */}
      <div className="flex-1 flex flex-col min-w-0">
        <textarea
          className="w-full h-24 rounded-md border p-3 font-mono text-sm bg-background shrink-0"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          spellCheck={false}
        />
        <div className="flex items-center gap-3 py-3 shrink-0">
          <button
            className="h-9 px-4 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90"
            onClick={handleRun}
            disabled={loading}
          >
            {loading ? "Ejecutando..." : "Ejecutar"}
          </button>
          <span className="text-xs text-muted-foreground">Solo SELECT — INSERT/UPDATE/DELETE/PRAGMA se rechazan.</span>
          {result && (
            <span className="text-xs text-muted-foreground ml-auto">
              {result.rows.length} filas{result.truncated ? " (truncado)" : ""}
            </span>
          )}
        </div>

        {error && (
          <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm mb-3 shrink-0">{error}</div>
        )}

        <div className="flex-1 overflow-auto border rounded-lg">
          {result && (
            <table className="w-full text-sm">
              <thead className="bg-muted/50 sticky top-0">
                <tr>
                  {result.columns.map((c) => (
                    <th key={c} className="text-left px-3 py-2 font-medium whitespace-nowrap">{c}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {result.rows.length === 0 ? (
                  <tr>
                    <td colSpan={result.columns.length} className="text-center py-8 text-muted-foreground text-sm">
                      Sin resultados
                    </td>
                  </tr>
                ) : (
                  result.rows.map((row, i) => (
                    <tr key={i} className="border-t hover:bg-muted/30">
                      {row.map((cell, j) => (
                        <td key={j} className="px-3 py-2 font-mono text-xs whitespace-nowrap max-w-xs truncate">
                          {cell === null ? <span className="text-muted-foreground">NULL</span> : String(cell)}
                        </td>
                      ))}
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          )}
        </div>

        <div className="flex items-center gap-2 pt-3 shrink-0">
          <button
            className="h-8 px-3 rounded-md border text-sm disabled:opacity-50"
            onClick={handlePrevPage}
            disabled={page === 0 || loading}
          >
            Anterior
          </button>
          <span className="text-xs text-muted-foreground">Página {page + 1}</span>
          <button
            className="h-8 px-3 rounded-md border text-sm disabled:opacity-50"
            onClick={handleNextPage}
            disabled={loading || !result || !result.truncated}
          >
            Siguiente
          </button>
        </div>
      </div>
    </div>
  );
}
