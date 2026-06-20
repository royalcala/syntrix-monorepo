import type { EntityDefinition } from "../fields/registry";

export const devicesEntity: EntityDefinition = {
  id: "devices",
  label: "Dispositivos",
  icon: "users",
  collection: null as never,
  fields: [
    { key: "node_id", label: "Node ID", type: "text", width: 180, editable: false, sortable: true },
    { key: "name", label: "Nombre", type: "text", width: 180, editable: true, sortable: true },
    { key: "person", label: "Persona", type: "text", width: 150, editable: true, sortable: true },
    { key: "role", label: "Rol", type: "select", width: 140, editable: true, sortable: true, options: [
      { label: "Admin", value: "admin" },
      { label: "Ventas", value: "sales" },
      { label: "Contabilidad", value: "contabilidad" },
      { label: "RH", value: "hr" },
    ]},
    { key: "active", label: "Activo", type: "boolean", width: 80, editable: true, sortable: true },
  ],
  views: [
    {
      id: "all", label: "Todos", filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["node_id", "name", "person", "role", "active"],
    },
    {
      id: "active", label: "Activos",
      filters: [{ field: "active", op: "eq", value: true }],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["node_id", "name", "person", "role", "active"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
      { key: "history", label: "Historial" },
    ],
  },
  searchFields: ["name", "person", "node_id"],
};
