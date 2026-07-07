import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@syntrix/ui/components/ui/button";
import { Eye, Star, Archive, Trash2, Search } from "lucide-react";
import { toast } from "sonner";

interface DrizzleResult {
  rows: string[][];
}

interface ViewRow {
  doc_id: string;
  sql: string;
  entity: string;
  created_by: string;
  tags: string;
  meta_json: string;
  change_time: number;
}

export function ViewsPage({ org }: { org: string }) {
  const [search, setSearch] = useState("");

  const { data: views, isLoading } = useQuery({
    queryKey: ["views", org],
    queryFn: async () => {
      const result = await invoke<DrizzleResult>("drizzle_execute", {
        sql: "SELECT doc_id, sql, entity, created_by, tags, meta_json, change_time FROM view_definitions WHERE org_id = ?1 ORDER BY change_time DESC",
        params: [org],
      });
      return (result?.rows || []).map((row: string[]) => ({
        doc_id: row[0],
        sql: row[1],
        entity: row[2],
        created_by: row[3],
        tags: row[4],
        meta_json: row[5],
        change_time: Number(row[6]),
      })) as ViewRow[];
    },
    enabled: !!org,
  });

  const filtered = views?.filter((v) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (
      v.entity.toLowerCase().includes(q) ||
      v.doc_id.toLowerCase().includes(q) ||
      v.tags.toLowerCase().includes(q)
    );
  });

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 p-3 border-b border-border">
        <Search size={16} className="text-muted-foreground" />
        <input
          className="flex-1 bg-transparent border-none outline-none text-sm"
          placeholder="Filtrar vistas por entidad, tags o ID..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <span className="text-xs text-muted-foreground">
          {filtered?.length ?? 0} vistas
        </span>
      </div>

      <div className="flex-1 overflow-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            Cargando vistas...
          </div>
        ) : !filtered || filtered.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            {search ? "Sin resultados" : "No hay vistas guardadas. Usa la IA para generar tu primera vista."}
          </div>
        ) : (
          <div className="divide-y divide-border">
            {filtered.map((view) => (
              <div key={view.doc_id} className="p-3 hover:bg-muted/50 transition-colors">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-primary/10 text-primary uppercase">
                        {view.entity}
                      </span>
                      {view.tags && (
                        <span className="text-xs text-muted-foreground truncate">
                          {view.tags}
                        </span>
                      )}
                    </div>
                    <code className="text-xs text-muted-foreground block line-clamp-2 font-mono">
                      {view.sql}
                    </code>
                    <div className="text-xs text-muted-foreground mt-1">
                      Creado por: {view.created_by.slice(0, 8)}...
                    </div>
                  </div>
                  <div className="flex gap-1 shrink-0">
                    <Button variant="ghost" size="sm" className="h-7 w-7 p-0"
                      title="Abrir vista"
                      onClick={async () => {
                        try {
                          await invoke("drizzle_execute", {
                            sql: String(view.sql),
                            params: [org],
                          });
                          toast.success("Vista ejecutada");
                        } catch (e: any) {
                          toast.error(e);
                        }
                      }}
                    >
                      <Eye size={14} />
                    </Button>
                    <Button variant="ghost" size="sm" className="h-7 w-7 p-0 text-muted-foreground"
                      title="Promover a vista default de la org"
                    >
                      <Star size={14} />
                    </Button>
                    <Button variant="ghost" size="sm" className="h-7 w-7 p-0 text-muted-foreground"
                      title="Archivar"
                    >
                      <Archive size={14} />
                    </Button>
                    <Button variant="ghost" size="sm" className="h-7 w-7 p-0 text-muted-foreground"
                      title="Eliminar"
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
