import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { ViewRenderer, type ViewDefinition } from "@syntrix/ui";
import { PageLayout } from "@syntrix/ui/components/PageLayout";
import { useParams } from "react-router-dom";

export function ViewScreen({ org }: { org: string }) {
  const { viewId } = useParams();

  const { data: view, isLoading: viewLoading } = useQuery({
    queryKey: ["view-def", org, viewId],
    queryFn: async () => {
      const rows = await invoke<any>("drizzle_execute", {
        sql: "SELECT sql, entity, components_json, root, meta_json FROM view_definitions WHERE org_id = ?1 AND doc_id = ?2 LIMIT 1",
        params: [org, viewId],
      });
      const row = rows?.rows?.[0];
      if (!row) throw new Error("View not found");
      return {
        id: viewId,
        orgId: org,
        sql: String(row[0]),
        entity: String(row[1]),
        components: JSON.parse(String(row[2])),
        root: String(row[3]),
        meta: JSON.parse(String(row[4])),
      } as ViewDefinition;
    },
    enabled: !!org && !!viewId,
  });

  const { data: queryData, isLoading: dataLoading } = useQuery({
    queryKey: ["view-data", org, viewId],
    queryFn: async () => {
      if (!view?.sql) return [];
      const result = await invoke<any>("drizzle_execute", {
        sql: view.sql,
        params: [org],
      });
      const rows: string[][] = result?.rows || [];
      if (rows.length === 0) return [];
      const numCols = Math.max(...rows.map(r => r.length));
      const cols: string[] = Array.from({ length: numCols }, (_, i) => `col_${i}`);
      return rows.map((row: string[]) => {
        const obj: Record<string, unknown> = {};
        cols.forEach((col, i) => { obj[col] = row[i] ?? null; });
        return obj;
      });
    },
    enabled: !!view?.sql,
  });

  if (viewLoading || dataLoading) {
    return (
      <PageLayout title="Cargando vista..." description="">
        <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
          Cargando...
        </div>
      </PageLayout>
    );
  }

  if (!view) {
    return (
      <PageLayout title="Vista no encontrada" description="">
        <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
          La vista solicitada no existe o ha sido eliminada.
        </div>
      </PageLayout>
    );
  }

  return (
    <PageLayout
      title={view.meta?.naturalLanguageQuery || "Vista"}
      description={`Entidad: ${view.entity} · ${queryData?.length || 0} registros`}
    >
      <ViewRenderer view={view} data={queryData || []} />
    </PageLayout>
  );
}
