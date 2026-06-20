import { createCollection } from "@tanstack/react-db";
import { tauriCollectionOptions } from "./tauri-adapter";
import { orgSchema } from "./schemas";

export function createOrgsCollection() {
  return createCollection(
    tauriCollectionOptions({
      id: "orgs",
      getKey: (o) => o.name as string,
      schema: orgSchema,
      listCommand: "list_orgs",
      mapRow: (item) => {
        const name = item as string;
        return { id: name, name, node_count: 0, created_at: new Date().toISOString() } as never;
      },
    }),
  );
}
