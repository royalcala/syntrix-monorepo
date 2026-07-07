import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { Button } from "@syntrix/ui/components/ui/button";
import { Card } from "@syntrix/ui/components/ui/card";
import { Send, Zap, Clock } from "lucide-react";
import { VoiceInput } from "../components/VoiceInput";
import { queueIAQuery } from "../hooks/useIAQueue";
import { toast } from "sonner";

interface ViewRow {
  doc_id: string;
  sql: string;
  entity: string;
  meta_json: string;
  change_time: number;
  tags: string;
}

export function HomeScreen({ org }: { org: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);

  const { data: recentViews } = useQuery({
    queryKey: ["home-views", org],
    queryFn: async () => {
      const result: any = await invoke("drizzle_execute", {
        sql: "SELECT doc_id, sql, entity, meta_json, change_time, tags FROM view_definitions WHERE org_id = ?1 ORDER BY change_time DESC LIMIT 5",
        params: [org],
      });
      return ((result as any)?.rows || []).map((row: string[]) => ({
        doc_id: row[0] ?? "",
        sql: row[1] ?? "",
        entity: row[2] ?? "",
        meta_json: row[3] ?? "{}",
        change_time: Number(row[4] ?? 0),
        tags: row[5] ?? "",
      })) as ViewRow[];
    },
    enabled: !!org,
  });

  const handleAIChat = async () => {
    if (!query.trim()) return;
    setLoading(true);
    try {
      const resp = await invoke<any>("ai_chat", {
        orgId: org,
        messages: [{ role: "user", content: query }],
        model: null,
      });
      if (resp.status === "Generated") {
        toast.success("Vista generada. Recarga la página para verla.");
      }
      queryClient.invalidateQueries({ queryKey: ["home-views", org] });
      setQuery("");
    } catch (e: any) {
      // If ai_chat fails, queue via CDC fallback
      queueIAQuery(org, query);
      toast.info("IA no disponible. La consulta se encoló y se procesará cuando la IA esté online.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-4 space-y-6 max-w-3xl mx-auto">
      <div>
        <h1 className="text-lg font-semibold mb-1">Inicio</h1>
        <p className="text-sm text-muted-foreground">
          Pregúntale a la IA lo que necesitas o explora tus vistas recientes.
        </p>
      </div>

      <div className="flex gap-2">
        <div className="flex-1 flex items-center gap-1 bg-background border border-border rounded-md px-3 focus-within:ring-1 focus-within:ring-ring">
          <VoiceInput onTranscript={(text) => setQuery(text)} disabled={loading} />
          <input
            className="flex-1 h-10 bg-transparent border-none outline-none text-sm"
            placeholder="Ej: facturas de mayo pendientes, o usa el micrófono..."
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleAIChat()}
          />
        </div>
        <Button onClick={handleAIChat} disabled={loading || !query.trim()}>
          {loading ? (
            <span className="flex items-center gap-1"><Zap size={14} className="animate-pulse" /> Pensando...</span>
          ) : (
            <span className="flex items-center gap-1"><Send size={14} /> Enviar</span>
          )}
        </Button>
      </div>

      <div>
        <h2 className="text-sm font-medium flex items-center gap-1.5 mb-3">
          <Clock size={14} className="text-muted-foreground" />
          Vistas Recientes
        </h2>
        {!recentViews || recentViews.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No hay vistas guardadas. Usa el campo de arriba para crear tu primera vista.
          </p>
        ) : (
          <div className="space-y-2">
            {recentViews.map(v => (
              <Card key={v.doc_id} className="p-3 cursor-pointer hover:bg-muted/50 transition-colors"
                onClick={() => navigate(`/view/${encodeURIComponent(v.doc_id)}`)}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <span className="text-xs font-mono px-1 py-0.5 rounded bg-primary/10 text-primary">
                      {v.entity}
                    </span>
                    {v.tags && <span className="text-xs text-muted-foreground ml-2">{v.tags}</span>}
                    <code className="block text-xs text-muted-foreground truncate mt-1 font-mono">
                      {v.sql}
                    </code>
                  </div>
                </div>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
