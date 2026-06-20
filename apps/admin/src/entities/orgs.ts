import type { EntityDefinition } from "../fields/registry";

export const orgsEntity: EntityDefinition = {
  id: "orgs",
  label: "Organizaciones",
  icon: "building",
  collection: null as never,
  fields: [
    { key: "name", label: "Nombre", type: "text", width: 200, editable: false, sortable: true },
    { key: "node_count", label: "Dispositivos", type: "number", width: 120, editable: false, sortable: true },
    { key: "created_at", label: "Creada", type: "date", width: 160, editable: false, sortable: true },
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
