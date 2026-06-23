import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const orgsEntity: EntityDefinition = {
  id: "orgs",
  label: "Organizaciones",
  icon: "building-2",
  loadData: (orgId?: string) => fetchEntityData("orgs", undefined, undefined, orgId),
  fields: [
    { key: "name", label: "Nombre", type: "text", width: 200, editable: false, sortable: true },
    { key: "node_count", label: "Dispositivos", type: "number", width: 120, editable: false, sortable: true },
    { key: "role", label: "Mi Rol", type: "text", width: 120, editable: false, sortable: true },
    { key: "created_at", label: "Creado", type: "date", width: 150, editable: false, sortable: true },
  ],
  views: [
    {
      id: "all", label: "Todas", filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["name", "node_count", "created_at"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
      { key: "devices", label: "Dispositivos" },
    ],
  },
  searchFields: ["name"],
};
