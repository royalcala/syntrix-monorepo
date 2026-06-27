import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { PageLayout } from "@syntrix/ui";
import { Card, CardContent, CardHeader, CardTitle } from "@syntrix/ui/components/ui/card";
import { Button } from "@syntrix/ui/components/ui/button";
import { Input } from "@syntrix/ui/components/ui/input";
import { Badge } from "@syntrix/ui/components/ui/badge";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@syntrix/ui/components/ui/select";
import {
  RefreshCw,
  Play,
  Pause,
  X,
  BarChart3,
  ChevronDown,
  ChevronRight,
} from "lucide-react";

interface LogRecord {
  ts: string;
  level: string;
  target: string;
  span_path: string;
  corr_id: string;
  message: string;
  fields: Record<string, unknown>;
}

interface LogQuery {
  level?: string;
  target?: string;
  span_path?: string;
  org?: string;
  since?: string;
  until?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

interface LogSummary {
  counts_by_level: Record<string, number>;
  counts_by_target: Record<string, number>;
  top_spans: string[];
  recent_errors: LogRecord[];
}

const LEVEL_COLORS: Record<string, string> = {
  TRACE: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300",
  DEBUG: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300",
  INFO: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300",
  WARN: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300",
  ERROR: "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300",
};

const LEVEL_OPTIONS = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];

const PAGE_SIZE = 200;

export function Logs() {
  const [records, setRecords] = useState<LogRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Filters
  const [filterLevel, setFilterLevel] = useState<string>("");
  const [filterTarget, setFilterTarget] = useState("");
  const [filterOrg, setFilterOrg] = useState("");
  const [filterSearch, setFilterSearch] = useState("");

  // Pagination
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  // Tail mode
  const [tailing, setTailing] = useState(false);
  const tailBufferRef = useRef<LogRecord[]>([]);
  const tailTimerRef = useRef<number | null>(null);

  // Summary
  const [summary, setSummary] = useState<LogSummary | null>(null);
  const [showSummary, setShowSummary] = useState(false);

  // Expanded rows
  const [expandedKey, setExpandedKey] = useState<string | null>(null);



  const buildQuery = useCallback(
    (overrides: Partial<LogQuery> = {}): LogQuery => {
      const q: LogQuery = {
        limit: PAGE_SIZE,
        offset: overrides.offset ?? offset,
        ...overrides,
      };
      if (filterLevel) q.level = filterLevel;
      if (filterTarget) q.target = filterTarget;
      if (filterOrg) q.org = filterOrg;
      if (filterSearch) q.search = filterSearch;
      return q;
    },
    [filterLevel, filterTarget, filterOrg, filterSearch, offset],
  );

  const loadLogs = useCallback(
    async (append = false) => {
      setLoading(true);
      setError("");
      try {
        const query = buildQuery(append ? { offset } : { offset: 0 });
        const results: LogRecord[] = await invoke("query_logs", { query });
        if (append) {
          setRecords((prev) => [...prev, ...results]);
        } else {
          setRecords(results);
          setOffset(0);
        }
        setHasMore(results.length >= PAGE_SIZE);
      } catch (e) {
        setError(String(e));
      }
      setLoading(false);
    },
    [buildQuery, offset],
  );

  const loadMore = useCallback(() => {
    const newOffset = offset + PAGE_SIZE;
    setOffset(newOffset);
    loadLogs(true);
  }, [offset, loadLogs]);

  const loadSummary = useCallback(async () => {
    try {
      const s: LogSummary = await invoke("summarize_logs", { windowSecs: 300 });
      setSummary(s);
    } catch (e) {
      console.error("summarize_logs failed:", e);
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadLogs(false);
    loadSummary();
  }, []);

  // Re-load when filters change
  useEffect(() => {
    loadLogs(false);
  }, [filterLevel, filterTarget, filterOrg, filterSearch]);

  // Tail mode: listen to log_event
  useEffect(() => {
    if (!tailing) return;

    let unlisten: UnlistenFn | undefined;

    const setup = async () => {
      unlisten = await listen<LogRecord>("log_event", (event) => {
        tailBufferRef.current.push(event.payload);

        // Batch flush every 250ms
        if (tailTimerRef.current === null) {
          tailTimerRef.current = window.setTimeout(() => {
            const batch = tailBufferRef.current.splice(0);
            setRecords((prev) => {
              const merged = [...batch, ...prev];
              return merged.slice(0, 1000); // bounded buffer
            });
            tailTimerRef.current = null;
          }, 250);
        }
      });
    };

    setup();

    return () => {
      if (unlisten) unlisten();
      if (tailTimerRef.current !== null) {
        clearTimeout(tailTimerRef.current);
        tailTimerRef.current = null;
      }
      tailBufferRef.current = [];
    };
  }, [tailing]);

  const toggleTail = useCallback(() => {
    if (tailing) {
      setTailing(false);
    } else {
      setRecords([]);
      setOffset(0);
      setTailing(true);
    }
  }, [tailing]);

  const clearFilters = useCallback(() => {
    setFilterLevel("");
    setFilterTarget("");
    setFilterOrg("");
    setFilterSearch("");
  }, []);

  const hasFilters = filterLevel || filterTarget || filterOrg || filterSearch;

  const rowKey = (r: LogRecord, i: number) => `${r.ts}-${r.corr_id}-${i}`;

  return (
    <PageLayout
      title="Logs del Sistema"
      description="Historial detallado de eventos de sincronización y comunicación P2P."
      actions={
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              loadSummary();
              setShowSummary(!showSummary);
            }}
            className="flex items-center gap-1.5"
          >
            <BarChart3 size={14} />
            Resumen
          </Button>
          <Button
            variant={tailing ? "default" : "outline"}
            size="sm"
            onClick={toggleTail}
            className="flex items-center gap-1.5"
          >
            {tailing ? <Pause size={14} /> : <Play size={14} />}
            {tailing ? "Detener" : "En Vivo"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => loadLogs(false)}
            disabled={loading || tailing}
            className="flex items-center gap-1.5"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            Actualizar
          </Button>
        </div>
      }
    >
      {/* Summary panel */}
      {showSummary && summary && (
        <Card className="mb-4">
          <CardHeader className="py-3">
            <CardTitle className="text-sm font-semibold flex items-center gap-2">
              <BarChart3 size={14} />
              Resumen (últimos 5 min)
            </CardTitle>
          </CardHeader>
          <CardContent className="py-2">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {/* Counts by level */}
              <div>
                <h4 className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">
                  Por Nivel
                </h4>
                <div className="space-y-1">
                  {LEVEL_OPTIONS.map((lvl) => (
                    <div key={lvl} className="flex items-center justify-between text-xs">
                      <Badge
                        variant="outline"
                        className={`${LEVEL_COLORS[lvl] || ""} text-xs px-2 py-0`}
                      >
                        {lvl}
                      </Badge>
                      <span className="font-mono tabular-nums">
                        {summary.counts_by_level[lvl] || 0}
                      </span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Top targets */}
              <div>
                <h4 className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">
                  Por Módulo (top 5)
                </h4>
                <div className="space-y-1">
                  {Object.entries(summary.counts_by_target)
                    .sort((a, b) => b[1] - a[1])
                    .slice(0, 5)
                    .map(([target, count]) => (
                      <div key={target} className="flex items-center justify-between text-xs">
                        <span className="font-mono truncate max-w-[150px]">{target}</span>
                        <span className="font-mono tabular-nums">{count}</span>
                      </div>
                    ))}
                </div>
              </div>

              {/* Recent errors */}
              <div>
                <h4 className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">
                  Errores Recientes
                </h4>
                <div className="space-y-1 max-h-[120px] overflow-y-auto">
                  {summary.recent_errors.length === 0 ? (
                    <span className="text-xs text-muted-foreground">Sin errores</span>
                  ) : (
                    summary.recent_errors.slice(0, 5).map((err, i) => (
                      <div key={i} className="text-xs font-mono text-red-600 dark:text-red-400 truncate">
                        {err.message}
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Filters */}
      <Card className="mb-4">
        <CardContent className="py-3">
          <div className="flex flex-wrap items-center gap-3">
            {/* Level filter */}
            <div className="w-28">
              <Select value={filterLevel} onValueChange={(v) => setFilterLevel(v === "all" ? "" : v)}>
                <SelectTrigger className="h-9 text-xs">
                  <SelectValue placeholder="Nivel" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">Todos</SelectItem>
                  {LEVEL_OPTIONS.map((lvl) => (
                    <SelectItem key={lvl} value={lvl}>
                      {lvl}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Target filter */}
            <Input
              className="h-9 w-40 text-xs"
              placeholder="Módulo"
              value={filterTarget}
              onChange={(e) => setFilterTarget(e.target.value)}
            />

            {/* Org filter */}
            <Input
              className="h-9 w-32 text-xs"
              placeholder="Org"
              value={filterOrg}
              onChange={(e) => setFilterOrg(e.target.value)}
            />

            {/* Text search */}
            <Input
              className="h-9 w-48 text-xs"
              placeholder="Buscar en mensajes..."
              value={filterSearch}
              onChange={(e) => setFilterSearch(e.target.value)}
            />

            {/* Clear filters */}
            {hasFilters && (
              <Button variant="ghost" size="sm" onClick={clearFilters} className="h-9">
                <X size={14} className="mr-1" />
                Limpiar
              </Button>
            )}

            {tailing && (
              <span className="text-xs text-green-600 dark:text-green-400 font-medium animate-pulse ml-auto">
                ● Recibiendo eventos en vivo...
              </span>
            )}

            <span className="text-xs text-muted-foreground ml-auto">
              {records.length} registros
            </span>
          </div>
        </CardContent>
      </Card>

      {/* Error banner */}
      {error && (
        <div className="mb-4 p-3 bg-destructive/10 border border-destructive/20 rounded-lg text-xs text-destructive">
          {error}
        </div>
      )}

      {/* Log table */}
      <Card>
        <CardContent className="p-0">
          <div className="max-h-[65vh] overflow-y-auto">
              <table className="w-full text-xs">
                <thead className="bg-muted/50 sticky top-0 z-10">
                  <tr>
                    <th className="text-left px-2 py-2 font-medium w-[160px]">Timestamp</th>
                    <th className="text-left px-2 py-2 font-medium w-[60px]">Nivel</th>
                    <th className="text-left px-2 py-2 font-medium w-[120px]">Módulo</th>
                    <th className="text-left px-2 py-2 font-medium w-[180px] hidden md:table-cell">
                      Span
                    </th>
                    <th className="text-left px-2 py-2 font-medium">Mensaje</th>
                    <th className="text-center px-2 py-2 font-medium w-[50px]">Detalle</th>
                  </tr>
                </thead>
                <tbody>
                  {records.length === 0 && !loading ? (
                    <tr>
                      <td colSpan={6} className="text-center py-8 text-muted-foreground text-sm">
                        No se encontraron registros
                      </td>
                    </tr>
                  ) : (
                    records.map((r, i) => {
                      const key = rowKey(r, i);
                      const isExpanded = expandedKey === key;
                      return (
                        <tr
                          key={key}
                          className="border-t border-muted/30 hover:bg-muted/20 transition-colors"
                        >
                          <td className="px-2 py-1.5 font-mono text-[10px] text-muted-foreground whitespace-nowrap">
                            {formatTimestamp(r.ts)}
                          </td>
                          <td className="px-2 py-1.5">
                            <Badge
                              variant="outline"
                              className={`${LEVEL_COLORS[r.level] || ""} text-[10px] px-1.5 py-0 leading-tight`}
                            >
                              {r.level}
                            </Badge>
                          </td>
                          <td className="px-2 py-1.5 font-mono text-[10px] text-muted-foreground truncate max-w-[120px]">
                            {r.target}
                          </td>
                          <td className="px-2 py-1.5 font-mono text-[10px] text-muted-foreground truncate max-w-[180px] hidden md:table-cell">
                            {r.span_path || (
                              <span className="italic opacity-50">—</span>
                            )}
                          </td>
                          <td className="px-2 py-1.5 font-mono text-[10px] truncate max-w-[300px]">
                            {r.message}
                          </td>
                          <td className="px-2 py-1.5 text-center">
                            <button
                              className="text-muted-foreground hover:text-foreground"
                              onClick={() => setExpandedKey(isExpanded ? null : key)}
                            >
                              {isExpanded ? (
                                <ChevronDown size={12} />
                              ) : (
                                <ChevronRight size={12} />
                              )}
                            </button>
                          </td>
                        </tr>
                      );
                    })
                  )}
                </tbody>
              </table>

              {/* Expanded detail rows */}
              {records.map((r, i) => {
                const key = rowKey(r, i);
                if (expandedKey !== key) return null;
                return (
                  <div
                    key={`detail-${key}`}
                    className="border-t border-muted/30 bg-muted/10 p-3 text-xs font-mono"
                  >
                    <div className="grid grid-cols-2 gap-2 mb-2">
                      <div>
                        <span className="text-muted-foreground">corr_id: </span>
                        {r.corr_id || "—"}
                      </div>
                      <div>
                        <span className="text-muted-foreground">span_path: </span>
                        {r.span_path || "—"}
                      </div>
                    </div>
                    {Object.keys(r.fields).length > 0 && (
                      <div>
                        <span className="text-muted-foreground">fields: </span>
                        <pre className="inline whitespace-pre-wrap break-all">
                          {JSON.stringify(r.fields, null, 2)}
                        </pre>
                      </div>
                    )}
                  </div>
                );
              })}

              {/* Loading indicator */}
              {loading && (
                <div className="flex items-center justify-center py-4 text-xs text-muted-foreground">
                  <RefreshCw size={12} className="animate-spin mr-2" />
                  Cargando...
                </div>
              )}

              {/* Load more */}
              {!loading && !tailing && hasMore && (
                <div className="flex justify-center py-3">
                  <Button variant="ghost" size="sm" onClick={loadMore} className="text-xs">
                    Cargar más ({PAGE_SIZE})
                  </Button>
                </div>
              )}
            </div>
        </CardContent>
      </Card>
    </PageLayout>
  );
}

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts);
    const hh = d.getHours().toString().padStart(2, "0");
    const mm = d.getMinutes().toString().padStart(2, "0");
    const ss = d.getSeconds().toString().padStart(2, "0");
    const ms = d.getMilliseconds().toString().padStart(3, "0");
    return `${hh}:${mm}:${ss}.${ms}`;
  } catch {
    return ts;
  }
}
