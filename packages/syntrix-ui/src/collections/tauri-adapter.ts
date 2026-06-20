import { invoke } from "@tauri-apps/api/core";
import type {
  CollectionConfig,
  InsertMutationFnParams,
  UpdateMutationFnParams,
  DeleteMutationFnParams,
  SyncConfig,
} from "@tanstack/db";

interface TauriCollectionConfig<TItem extends { id: string | number }> {
  id: string;
  schema: CollectionConfig<TItem>["schema"];
  getKey: (item: TItem) => string | number;
  listCommand: string;
  listArgs?: Record<string, unknown>;
  insertCommand?: string;
  updateCommand?: string;
  deleteCommand?: string;
  mapRow?: (item: unknown) => TItem;
}

export function tauriCollectionOptions<TItem extends { id: string | number }>(
  config: TauriCollectionConfig<TItem>,
): CollectionConfig<TItem> {
  const mapItem = config.mapRow ?? ((item: unknown) => item as TItem);

  // Captured sync callbacks for confirming mutations locally
  let syncBegin: (() => void) | null = null;
  let syncWrite: ((msg: { type: string; value?: unknown; key?: unknown }) => void) | null = null;
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
              await invoke(config.insertCommand!, { ...config.listArgs, ...m.modified as Record<string, unknown> });
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
              await invoke(config.updateCommand!, { ...config.listArgs, key: m.key, changes: m.changes });
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
