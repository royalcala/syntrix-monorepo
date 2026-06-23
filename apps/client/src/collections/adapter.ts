import { invoke } from "@tauri-apps/api/core";

export async function fetchEntityData<T>(
  entityId: string,
  filterField?: string,
  filterValue?: string
): Promise<T[]> {
  try {
    const result = await invoke<T[]>("query_entity", {
      entity: entityId,
      filterField: filterField ?? null,
      filterValue: filterValue ?? null,
    });
    return result;
  } catch (err) {
    console.error(`Failed to fetch ${entityId}:`, err);
    return [];
  }
}
