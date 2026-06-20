import { createCollection } from "@tanstack/react-db";
import { tauriCollectionOptions } from "./tauri-adapter";
import { deviceSchema } from "./schemas";

export function createDevicesCollection(org: string) {
  return createCollection(
    tauriCollectionOptions({
      id: "devices",
      getKey: (d) => d.node_id as string,
      schema: deviceSchema,
      listCommand: "list_devices",
      listArgs: { org },
      insertCommand: "add_device",
      updateCommand: "update_device",
      mapRow: (item) => {
        const d = item as { node_id: string; name: string; person: string; role: string; active: boolean };
        return { id: d.node_id, node_id: d.node_id, name: d.name, person: d.person, role: d.role, active: d.active } as never;
      },
    }),
  );
}
