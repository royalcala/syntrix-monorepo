import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

/**
 * Polls ia_queries with status "completed" for the active org.
 * When one is found, shows a notification and invalidates the views cache.
 */
export function useIAQueue(orgId: string | undefined) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!orgId) return;

    let running = true;
    let seenIds = new Set<string>();

    const poll = async () => {
      while (running) {
        try {
          const result: any = await invoke("drizzle_execute", {
            sql: "SELECT doc_id, view_id, text, status FROM ia_queries WHERE org_id = ?1 AND status = 'completed' ORDER BY change_time ASC LIMIT 5",
            params: [orgId],
          });
          const rows: string[][] = result?.rows || [];

          for (const row of rows) {
            const docId = row[0];
            const viewId = row[1];
            const text = row[2] || "consulta";

            if (seenIds.has(docId)) continue;
            seenIds.add(docId);

            toast.success(`Vista "${text.slice(0, 50)}" lista`, {
              description: "Tu consulta fue procesada por la IA.",
              action: viewId ? {
                label: "Abrir",
                onClick: () => {
                  window.location.href = `/view/${encodeURIComponent(viewId)}`;
                },
              } : undefined,
            });

            // Mark as seen via upsert_entity (stamps change_time + node_id)
            await invoke("upsert_entity", {
              orgId,
              entity: "ia_queries",
              docId,
              fields: { text, status: "seen" },
            }).catch(() => {});
          }

          if (rows.length > 0) {
            queryClient.invalidateQueries({ queryKey: ["home-views", orgId] });
          }
        } catch {
          // DB may not be ready
        }

        await new Promise((r) => setTimeout(r, 5000));
      }
    };

    poll();

    return () => { running = false; };
  }, [orgId, queryClient]);
}

/**
 * Queues a query in ia_queries when ai_chat fails or times out.
 * Uses upsert_entity which stamps change_time and node_id automatically.
 */
export async function queueIAQuery(orgId: string, text: string): Promise<void> {
  const docId = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`;

  await invoke("upsert_entity", {
    orgId,
    entity: "ia_queries",
    docId,
    fields: { text, status: "pending" },
  }).catch((e: any) => {
    console.error("Failed to queue IA query:", e);
  });
}
