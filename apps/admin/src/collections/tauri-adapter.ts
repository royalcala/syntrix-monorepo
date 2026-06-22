import { invoke } from "@tauri-apps/api/core";
import type {
  InsertMutationFnParams,
  UpdateMutationFnParams,
  DeleteMutationFnParams,
  SyncConfig,
} from "@tanstack/db";

interface TauriCollectionConfig<
  TItem extends { id: string | number },
  TSchema = any,
> {
  id: string;
  schema: TSchema;
  getKey: (item: TItem) => string | number;
  listCommand: string;
  listArgs?: Record<string, unknown>;
  insertCommand?: string;
  updateCommand?: string;
  deleteCommand?: string;
  mapRow?: (item: unknown) => TItem;
}

export function tauriCollectionOptions<
  TItem extends { id: string | number },
>(
  config: TauriCollectionConfig<TItem, any>,
): any {
  const mapItem = config.mapRow ?? ((item: unknown) => item as TItem);

  // Captured sync callbacks for confirming mutations locally
  let syncBegin: (() => void) | null = null;
  let syncWrite: ((msg: any) => void) | null = null;
  let syncCommit: (() => void) | null = null;

  return {
    getKey: config.getKey,
    id: config.id,
    schema: config.schema,
    startSync: true, // triggers sync() immediately
    gcTime: 0,       // don't aggressively GC
    
    sync: {
      sync: ({ begin, write, commit, markReady }) => {
        // Capture for confirming mutations
        syncBegin = begin;
        syncWrite = write;
        syncCommit = commit;

        // Load initial data from Tauri backend
        invoke<unknown[]>(config.listCommand, config.listArgs ?? {})
          .then((items) => {
            begin();
            for (const item of items) {
              write({ type: "insert", value: mapItem(item) });
            }
            commit();
          })
          .catch((err: Error) => {
            console.error(`[tauriCollection] sync failed for ${config.id}:`, err);
          })
          .finally(() => {
            markReady();
          });
      },
    } as SyncConfig<TItem, string | number>,

    onInsert: config.insertCommand
      ? async (params: InsertMutationFnParams<TItem>) => {
          for (const m of params.transaction.mutations) {
            if (m.type === "insert") {
              let payload: Record<string, unknown> = {};
              if (config.id === "devices") {
                const item = m.modified as any;
                payload = {
                  org: config.listArgs?.org,
                  nodeId: item.node_id,
                  name: item.name || (item.node_id as string).slice(0, 12),
                  person: item.person || "user",
                  role: item.role,
                  deviceAddr: item.device_addr || "",
                };
              } else {
                payload = { ...config.listArgs, ...m.modified as Record<string, unknown> };
              }
              await invoke(config.insertCommand!, payload);
              // Confirm locally to move from optimistic → synced
              if (syncBegin && syncWrite && syncCommit) {
                syncBegin();
                syncWrite({ type: "insert", value: m.modified as unknown });
                syncCommit();
              }
            }
          }
        }
      : undefined,

    onUpdate: config.updateCommand
      ? async (params: UpdateMutationFnParams<TItem>) => {
          for (const m of params.transaction.mutations) {
            if (m.type === "update") {
              let payload: Record<string, unknown> = {};
              if (config.id === "devices") {
                payload = {
                  org: config.listArgs?.org,
                  nodeId: m.key,
                  active: (m.modified as any).active,
                  role: (m.modified as any).role,
                  name: (m.modified as any).name,
                  person: (m.modified as any).person,
                };
              } else {
                payload = { ...config.listArgs, key: m.key, changes: m.changes };
              }
              await invoke(config.updateCommand!, payload);
              if (syncBegin && syncWrite && syncCommit) {
                syncBegin();
                syncWrite({ type: "update", key: m.key, value: m.modified as unknown });
                syncCommit();
              }
            }
          }
        }
      : undefined,

    onDelete: config.deleteCommand
      ? async (params: DeleteMutationFnParams<TItem>) => {
          for (const m of params.transaction.mutations) {
            if (m.type === "delete") {
              await invoke(config.deleteCommand!, { ...config.listArgs, key: m.key });
              if (syncBegin && syncWrite && syncCommit) {
                syncBegin();
                syncWrite({ type: "delete", key: m.key });
                syncCommit();
              }
            }
          }
        }
      : undefined,
  };
}
