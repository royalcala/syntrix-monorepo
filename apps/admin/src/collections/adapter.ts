import { invoke } from "@tauri-apps/api/core";

export async function fetchEntityData<T>(
  entityId: string,
  filterField?: string,
  filterValue?: string
): Promise<T[]> {
  try {
    const org = localStorage.getItem("syntrix_admin_org") || "";
    if (entityId === "devices") return await invoke<T[]>("list_devices", { org });
    if (entityId === "roles") return await invoke<T[]>("list_roles", { org });
    if (entityId === "orgs") return await invoke<T[]>("list_orgs");
    return [];
  } catch (err) {
    console.error(`Failed to fetch ${entityId}:`, err);
    return [];
  }
}
