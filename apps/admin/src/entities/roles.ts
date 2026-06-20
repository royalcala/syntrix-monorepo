import type { EntityDefinition } from "../fields/registry";

export const rolesEntity: EntityDefinition = {
  id: "roles",
  label: "Roles",
  icon: "shield",
  collection: null as never,
  fields: [
    { key: "name", label: "Nombre", type: "text", width: 180, editable: false, sortable: true },
    { key: "can_open", label: "Lectura", type: "text", width: 300, editable: true, sortable: false },
    { key: "can_write", label: "Escritura", type: "text", width: 300, editable: true, sortable: false },
  ],
  views: [
    {
      id: "all", label: "Todos", filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["name", "can_open", "can_write"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
    ],
  },
  searchFields: ["name"],
};
