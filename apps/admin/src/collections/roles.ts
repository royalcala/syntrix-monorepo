import { createCollection } from "@tanstack/react-db";
import { tauriCollectionOptions } from "./tauri-adapter";
import { roleSchema } from "./schemas";

export function createRolesCollection(org: string) {
  return createCollection(
    tauriCollectionOptions({
      id: "roles",
      getKey: (r) => r.name as string,
      schema: roleSchema,
      listCommand: "list_roles",
      listArgs: { org },
      mapRow: (item) => {
        const r = item as { name: string; can_open: string[]; can_write: string[] };
        return { id: r.name, name: r.name, can_open: r.can_open.join(", "), can_write: r.can_write.join(", ") } as never;
      },
    }),
  );
}
