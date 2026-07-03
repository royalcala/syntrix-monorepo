import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";

interface CursorCdcEvent {
  change_type: number;
  table: string;
  change_time: number;
  columns: Record<string, unknown>;
}

interface CursorResponse {
  events: CursorCdcEvent[];
  max_change_id: number;
}

const CURSOR_KEY_PREFIX = "syntrix-timeline-cursor-";

function getCursor(orgId: string): number {
  try {
    return Number(localStorage.getItem(CURSOR_KEY_PREFIX + orgId)) || 0;
  } catch {
    return 0;
  }
}

function setCursor(orgId: string, cursor: number): void {
  try {
    localStorage.setItem(CURSOR_KEY_PREFIX + orgId, String(cursor));
  } catch { /* silent */ }
}

const TABLE_TO_ENTITY: Record<string, string> = {
  customers: "customers",
  suppliers: "suppliers",
  products: "products",
  invoices: "invoices",
  invoice_items: "invoice_items",
  orders: "orders",
  order_items: "order_items",
  payroll: "payroll",
};

/**
 * Polls `get_updates_since` for a given org and invalidates react-query
 * caches when new CDC events arrive. Stores the cursor in localStorage
 * so no events are missed across sessions.
 */
export function useTimelineCursor(orgId: string | undefined) {
  const queryClient = useQueryClient();
  const cursorRef = useRef(orgId ? getCursor(orgId) : 0);

  useEffect(() => {
    if (!orgId) return;

    cursorRef.current = getCursor(orgId);
    let running = true;

    const poll = async () => {
      while (running) {
        try {
          const raw = await invoke<CursorResponse>("get_updates_since", {
            orgId,
            sinceChangeId: cursorRef.current,
          });

          if (raw.events.length > 0) {
            cursorRef.current = raw.max_change_id;
            setCursor(orgId, raw.max_change_id);

            // Collect affected entity types
            const affected = new Set<string>();
            for (const ev of raw.events) {
              const entity = TABLE_TO_ENTITY[ev.table];
              if (entity) affected.add(entity);
              // For child tables, also invalidate parent
              if (ev.table === "invoice_items") affected.add("invoices");
              if (ev.table === "order_items") affected.add("orders");
            }

            // Invalidate entity queries so they refetch on next render
            for (const entity of affected) {
              queryClient.invalidateQueries({ queryKey: ["entity", entity] });
            }
            // Always invalidate the global entity cache too
            queryClient.invalidateQueries({ queryKey: ["entity"] });
          }
        } catch {
          // Connection may not be ready yet; retry silently
        }

        await new Promise((r) => setTimeout(r, 2000));
      }
    };

    poll();

    return () => { running = false; };
  }, [orgId, queryClient]);
}
