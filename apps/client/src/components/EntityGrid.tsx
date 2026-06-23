// @ts-nocheck
import { useState, useCallback, useEffect, useMemo } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  getFilteredRowModel,
  flexRender,
  type SortingState,
  type ColumnFiltersState,
  type VisibilityState,
} from "@tanstack/react-table";
import { Plus, Search, ArrowUp, ArrowDown } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useSearchParams } from "react-router-dom";
import { Table, TableHeader, TableBody, TableHead, TableRow, TableCell } from "./ui/table";
import type { EntityDefinition } from "../fields/registry";
import { DetailPanel } from "./DetailPanel";

interface EntityGridProps {
  entity: EntityDefinition & { loadData?: () => Promise<Array<Record<string, unknown>>> };
  activeView?: string;
  role?: string;
  orgId?: string;
  onSaveCreate?: (row: Row) => Promise<Row>;
}

interface Row { id: string; [key: string]: unknown; }

export function EntityGrid({ entity, activeView, role, orgId, onSaveCreate }: EntityGridProps) {
  const queryClient = useQueryClient();
  const [selectedRow, setSelectedRow] = useState<Row | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailMode, setDetailMode] = useState<"edit" | "create">("edit");
  const [viewId, setViewId] = useState(activeView ?? entity.views[0]?.id ?? "all");
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState("");
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({});

  const view = entity.views.find((v) => v.id === viewId) ?? entity.views[0];
  const visibleCols = view?.visibleColumns ?? entity.fields.map((f) => f.key);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedSearchQuery(searchQuery);
    }, 150);
    return () => clearTimeout(handler);
  }, [searchQuery]);

  // Reactive search results from Tantivy full-text index
  const { data: searchResults } = useQuery({
    queryKey: ["search", entity.id, orgId, debouncedSearchQuery],
    queryFn: async () => {
      if (!debouncedSearchQuery.trim()) return null;
      try {
        const results = await invoke<any[]>("search_entity", {
          query: debouncedSearchQuery,
          entities: [entity.id],
        });
        return results;
      } catch (err) {
        console.error("Search failed:", err);
        return [];
      }
    },
    enabled: !!debouncedSearchQuery.trim(),
  });

  // Reactive data from Tauri backend via Relational Engine
  const { data: liveData, isLoading } = useQuery({
    queryKey: ["entity", entity.id, orgId, viewId, columnFilters],
    queryFn: async () => {
      if (entity.loadData) return await entity.loadData();
      return [];
    },
  });

  const allRows = useMemo(() => {
    const raw = (liveData as Array<Record<string, unknown>> | undefined) ?? [];
    return raw.map((r) => ({ ...r, id: (r.id ?? crypto.randomUUID()) as string })) as Row[];
  }, [liveData]);

  const displayedRows = useMemo(() => {
    if (!debouncedSearchQuery.trim() || !searchResults) {
      return allRows;
    }
    const scoreMap = new Map(searchResults.map((r: any, idx: number) => [r.doc_id, idx]));
    return allRows
      .filter((row) => scoreMap.has(row.id))
      .sort((a, b) => scoreMap.get(a.id)! - scoreMap.get(b.id)!);
  }, [allRows, debouncedSearchQuery, searchResults]);

  const [searchParams, setSearchParams] = useSearchParams();
  const selectId = searchParams.get("id");

  useEffect(() => {
    if (selectId && allRows.length > 0) {
      const row = allRows.find((r) => r.id === selectId);
      if (row) {
        setSelectedRow(row);
        setDetailMode("edit");
        setDetailOpen(true);
        // Clear param so it doesn't reopen if closed
        const newParams = new URLSearchParams(searchParams);
        newParams.delete("id");
        setSearchParams(newParams, { replace: true });
      }
    }
  }, [selectId, allRows, searchParams, setSearchParams]);

  const columns = useMemo(() =>
    visibleCols.map((key) => {
      const field = entity.fields.find((f) => f.key === key);
      return {
        id: key,
        accessorKey: key,
        header: field?.label ?? key,
        enableSorting: field?.sortable ?? true,
        cell: ({ getValue, row }: { getValue: () => unknown; row: { original: Row } }) => {
          const value = getValue();
          if (value == null) return <span className="text-muted-foreground/40">—</span>;

          if (field?.type === "status") {
            const colors: Record<string, string> = {
              draft: "bg-muted text-muted-foreground", open: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
              paid: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300", cancelled: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
              pending: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
            };
            return <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${colors[String(value)] ?? "bg-muted text-muted-foreground"}`}>{value as string}</span>;
          }

          if (field?.type === "boolean") return value ? <span className="text-green-600 font-medium">✓</span> : <span className="text-muted-foreground/30">—</span>;

          if (field?.type === "currency") {
            const num = Number(value ?? 0);
            return <span className={`tabular-nums ${num < 0 ? "text-red-600" : ""}`}>${num.toFixed(2)}</span>;
          }

          if (field?.type === "number") return <span className="tabular-nums">{Number(value).toLocaleString()}</span>;

          if (field?.type === "date") {
            const d = value instanceof Date ? value : new Date(String(value));
            if (isNaN(d.getTime())) return <span className="text-muted-foreground/40">—</span>;
            return <span>{d.toLocaleDateString("es-MX", { day: "numeric", month: "short", year: "numeric" })}</span>;
          }

          return <span className="truncate">{String(value)}</span>;
        },
      };
    }),
    [visibleCols, entity.fields],
  );

  const table = useReactTable({
    data: displayedRows,
    columns,
    state: { sorting, columnFilters, columnVisibility },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getRowId: (row) => row.id,
  });


  const onCreateRecord = useCallback(async () => {
    const newId = crypto.randomUUID?.() ?? `${Date.now()}`;
    const newRow: Row = { id: newId };
    entity.fields.forEach((f) => {
      if (f.key !== "id") {
        if (f.type === "number" || f.type === "currency") newRow[f.key] = 0 as unknown;
        else if (f.type === "boolean") newRow[f.key] = false as unknown;
        else if (f.type === "status") newRow[f.key] = f.options?.[0]?.value ?? "draft" as unknown;
        else if (f.type === "date") newRow[f.key] = new Date().toISOString() as unknown;
        else newRow[f.key] = "" as unknown;
      }
    });
    setSelectedRow(newRow);
    setDetailMode("create");
    setDetailOpen(true);
  }, [entity]);

  const onRowClick = useCallback((row: Row) => {
    setSelectedRow({ ...row });
    setDetailMode("edit");
    setDetailOpen(true);
  }, []);

  useHotkeys([
    { hotkey: "Escape", callback: () => { if (detailOpen) setDetailOpen(false); } },
    { hotkey: "mod+n", callback: (e) => { e.preventDefault(); onCreateRecord(); } },
  ]);

  const rows = table.getRowModel().rows;

  return (
    <div className="flex h-full">
      <div className="flex-1 min-w-0 flex flex-col">
        <div className="flex items-center gap-2 px-4 py-2 border-b bg-card/30">
          {entity.views.length > 1 && (
            <div className="flex rounded-md border overflow-hidden text-xs">
              {entity.views.map((v) => (
                <button key={v.id} onClick={() => setViewId(v.id)}
                  className={`px-2.5 py-1 transition-colors ${viewId === v.id ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
                  {v.label}
                </button>
              ))}
            </div>
          )}
          <div className="relative ml-2">
            <Search className="absolute left-2 top-1.5 w-3.5 h-3.5 text-muted-foreground" />
            <input
              className="w-44 pl-7 pr-2 py-1 text-xs rounded-md border bg-background focus:outline-none focus:ring-1 focus:ring-primary"
              placeholder="Buscar..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
          <button onClick={onCreateRecord}
            className="px-3 py-1 text-xs font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors ml-auto">
            <Plus className="w-3 h-3 inline mr-1" />Nuevo
          </button>
          <span className="text-xs text-muted-foreground">{rows.length} registros</span>
        </div>

        <div className="flex-1 min-h-0 overflow-auto">
          {isLoading ? (
            <div className="p-4 space-y-2">
              {Array.from({ length: 12 }).map((_, i) => (
                <div key={i} className="h-8 bg-muted/50 rounded animate-pulse" style={{ width: `${60 + Math.random() * 40}%` }} />
              ))}
            </div>
          ) : rows.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground text-sm gap-3">
              {searchQuery ? (
                <>
                  <Search className="w-8 h-8 opacity-30" />
                  <span>No hay resultados para "{searchQuery}"</span>
                  <button onClick={() => setSearchQuery("")} className="text-xs text-primary hover:underline">Limpiar búsqueda</button>
                </>
              ) : (
                <>
                  <div className="w-12 h-12 rounded-full bg-muted/50 flex items-center justify-center">
                    <Plus className="w-6 h-6 opacity-40" />
                  </div>
                  <span>Aún no hay {entity.label.toLowerCase()}</span>
                  <button onClick={onCreateRecord} className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90">
                    Crear primer registro
                  </button>
                </>
              )}
            </div>
          ) : (
            <Table>
              <TableHeader>
                {table.getHeaderGroups().map((headerGroup) => (
                  <TableRow key={headerGroup.id}>
                    {headerGroup.headers.map((header) => (
                      <TableHead key={header.id} style={{ width: entity.fields.find((f) => f.key === header.id)?.width }}
                        className={header.column.getCanSort() ? "cursor-pointer select-none" : ""}
                        onClick={header.column.getToggleSortingHandler()}>
                        <div className="flex items-center gap-1">
                          {flexRender(header.column.columnDef.header, header.getContext())}
                          {{
                            asc: <ArrowUp className="w-3 h-3" />,
                            desc: <ArrowDown className="w-3 h-3" />,
                          }[header.column.getIsSorted() as string] ?? null}
                        </div>
                      </TableHead>
                    ))}
                  </TableRow>
                ))}
              </TableHeader>
              <TableBody>
                {rows.map((row) => (
                  <TableRow key={row.id} onClick={() => onRowClick(row.original)} className="cursor-pointer">
                    {row.getVisibleCells().map((cell) => (
                      <TableCell key={cell.id}>
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </div>
      </div>

      {detailOpen && selectedRow && (
        <>
          {/* Desktop: right-side panel */}
          <div className="hidden lg:block w-[420px] border-l bg-background flex flex-col shrink-0">
            <DetailPanel
        entity={entity}
                  row={selectedRow}
                  role={role}
                  isCreate={detailMode === "create"}
                  onSaveCreate={onSaveCreate || (async (val) => {
                    await invoke("commit_event", { eventType: `${entity.id}.created`, payload: JSON.stringify(val) });
                    await new Promise(r => setTimeout(r, 250)); // Margen seguro
                    await queryClient.refetchQueries({ queryKey: ["entity"] });
                    return val as Row;
                  })}
                  onSaveUpdate={async (id, val) => {
                    await invoke("commit_event", { eventType: `${entity.id}.updated`, payload: JSON.stringify(val) });
                    setSelectedRow((prev) => prev && prev.id === id ? { ...prev, ...val } as Row : prev);
                    // Actualización instantánea en memoria (Pessimistic Update)
                    queryClient.setQueryData(
                      ["entity", entity.id, orgId, viewId, columnFilters],
                      (old: any[]) => old ? old.map((r) => (r.id === id ? { ...r, ...val } : r)) : old
                    );
                    await new Promise(r => setTimeout(r, 250));
                    await queryClient.refetchQueries({ queryKey: ["entity"] });
                  }}
                  onClose={() => { setDetailOpen(false); }}
                  onNavigate={detailMode === "create" ? undefined : (dir) => {
                const idx = rows.findIndex((r) => r.original.id === selectedRow.id);
                const next = idx + dir;
                if (next >= 0 && next < rows.length) {
                  setSelectedRow({ ...rows[next]!.original });
                  setDetailMode("edit");
                }
              }}
            />
          </div>
          {/* Mobile: bottom sheet */}
          <div className="lg:hidden fixed inset-0 z-50">
            <div className="fixed inset-0 bg-black/50" onClick={() => { setDetailOpen(false); if (detailMode === "create") { setRefreshKey((k) => k + 1); } }} />
            <div className="fixed bottom-0 left-0 right-0 max-h-[90vh] bg-background rounded-t-xl border-t shadow-xl overflow-auto animate-in slide-in-from-bottom">
              <DetailPanel
                entity={entity}
                row={selectedRow}
                role={role}
                isCreate={detailMode === "create"}
                onSaveCreate={onSaveCreate || (async (val) => {
                  await invoke("commit_event", { eventType: `${entity.id}.created`, payload: JSON.stringify(val) });
                  await new Promise(r => setTimeout(r, 250));
                  await queryClient.refetchQueries({ queryKey: ["entity"] });
                  return val as Row;
                })}
                onSaveUpdate={async (id, val) => {
                  await invoke("commit_event", { eventType: `${entity.id}.updated`, payload: JSON.stringify(val) });
                  setSelectedRow((prev) => prev && prev.id === id ? { ...prev, ...val } as Row : prev);
                  queryClient.setQueryData(
                    ["entity", entity.id, orgId, viewId, columnFilters],
                    (old: any[]) => old ? old.map((r) => (r.id === id ? { ...r, ...val } : r)) : old
                  );
                  await new Promise(r => setTimeout(r, 250));
                  await queryClient.refetchQueries({ queryKey: ["entity"] });
                }}
                onClose={() => { setDetailOpen(false); }}
              />
            </div>
          </div>
        </>
      )}
    </div>
  );
}
