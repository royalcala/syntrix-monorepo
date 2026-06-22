import { z } from "zod";
import { createCollection } from "@tanstack/react-db";
import { tauriCollectionOptions } from "./tauri-adapter";
import { roleSchema } from "./schemas";

type Role = z.infer<typeof roleSchema> & { id: string };

export function createRolesCollection(org: string) {
  return createCollection(
    tauriCollectionOptions<Role>({
      id: "roles",
      getKey: (r) => r.name as string,
      schema: roleSchema,
      listCommand: "list_roles",
      insertCommand: "create_role",
      updateCommand: "update_role",
      listArgs: { org },
      mapRow: (item) => {
        const r = item as { name: string; can_open: string[]; can_write: string[] };
        return { id: r.name, name: r.name, can_open: r.can_open || [], can_write: r.can_write || [] };
      },
    }),
  );
}
