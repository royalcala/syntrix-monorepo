import { invoke } from "@tauri-apps/api/core";

export async function fetchEntityData<T>(
  entityId: string,
  _filterField?: string,
  _filterValue?: string,
  orgId?: string,
): Promise<T[]> {
  try {
    const result = await invoke<T[]>("query_entity", {
      orgId: orgId ?? null,
      entity: entityId,
      filterField: _filterField ?? null,
      filterValue: _filterValue ?? null,
    });
    return Array.isArray(result) ? result : [];
  } catch (err) {
    console.error(`Failed to fetch ${entityId}:`, err);
    return [];
  }
}
