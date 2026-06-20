import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CollectionConfig,
  SyncConfig,
  InsertMutationFnParams,
  UpdateMutationFnParams,
  DeleteMutationFnParams,
} from "@tanstack/db";

interface IrohCollectionConfig<TItem extends { id: string | number }> {
  dataType: string;
  schema: CollectionConfig<TItem>["schema"];
  getKey: (item: TItem) => string | number;
  roles: {
    canOpen: string[];
    canWrite: string[];
  };
}

export function irohCollectionOptions<TItem extends { id: string | number }>(
  config: IrohCollectionConfig<TItem>,
): CollectionConfig<TItem> {
  const sync: SyncConfig<TItem>["sync"] = (params) => {
    const { begin, write, commit, markReady } = params;

    async function initialSync() {
      try {
        begin();
        // Pull all entries from operational namespace for this dataType
        const result = await invoke<{
          batch: Array<{
            eventEncoded: {
              type: string;
              hlc: { ts: number; count: number; node: string };
              payload: TItem;
            };
          }>;
          hasMore: boolean;
          cursor: unknown;
        }>("sync_pull", {
          orgId: "",
          cursor: null,
        });

        for (const entry of result.batch) {
          const payload = entry.eventEncoded.payload as TItem;
          if (typeof payload === "object" && payload !== null) {
            write({
              type: "insert",
              value: {
                ...payload,
                _namespace: config.dataType,
                _author: entry.eventEncoded.hlc.node,
              } as unknown as TItem,
            });
          }
        }
        commit();
      } catch (err) {
        console.error(`[irohCollection:${config.dataType}] initial sync failed:`, err);
      } finally {
        markReady();
      }
    }

    initialSync();

    const unlisten = listen<{
      org_id: string;
      event: {
        type: string;
        hlc: { ts: number; count: number; node: string };
        payload: TItem;
      };
    }>("data-changed", (event) => {
      const { event: syncEvent } = event.payload;
      const eventType = syncEvent.type.split(".")[0];
      if (eventType === config.dataType) {
        begin();
        write({
          type: "insert",
          value: {
            ...syncEvent.payload,
            _namespace: config.dataType,
            _author: syncEvent.hlc.node,
          } as unknown as TItem,
        });
        commit();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  };

  const onInsert = async (params: InsertMutationFnParams<TItem>) => {
    for (const mutation of params.transaction.mutations) {
      if (mutation.type === "insert") {
        const isValid = config.roles.canWrite.some(
          (ns) => ns === config.dataType,
        );
        if (!isValid) {
          throw new Error(
            `Write denied: role cannot write to ${config.dataType}`,
          );
        }
        await invoke("commit_event", {
          eventType: `${config.dataType}.created`,
          payload: JSON.stringify(mutation.modified),
        });
      }
    }
  };

  const onUpdate = async (params: UpdateMutationFnParams<TItem>) => {
    for (const mutation of params.transaction.mutations) {
      if (mutation.type === "update") {
        const isValid = config.roles.canWrite.some(
          (ns) => ns === config.dataType,
        );
        if (!isValid) {
          throw new Error(
            `Write denied: role cannot write to ${config.dataType}`,
          );
        }
        await invoke("commit_event", {
          eventType: `${config.dataType}.updated`,
          payload: JSON.stringify({
            id: mutation.key,
            changes: mutation.changes,
          }),
        });
      }
    }
  };

  const onDelete = async (params: DeleteMutationFnParams<TItem>) => {
    for (const mutation of params.transaction.mutations) {
      if (mutation.type === "delete") {
        const isValid = config.roles.canWrite.some(
          (ns) => ns === config.dataType,
        );
        if (!isValid) {
          throw new Error(
            `Write denied: role cannot write to ${config.dataType}`,
          );
        }
        await invoke("commit_event", {
          eventType: `${config.dataType}.deleted`,
          payload: JSON.stringify({ id: mutation.key }),
        });
      }
    }
  };

  return {
    id: config.dataType,
    schema: config.schema,
    getKey: config.getKey,
    sync: { sync },
    onInsert,
    onUpdate,
    onDelete,
  };
}
