import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const devicesEntity: EntityDefinition = {
  id: "devices",
  label: "Dispositivos",
  icon: "users",
  loadData: (orgId?: string) => fetchEntityData("devices", undefined, undefined, orgId),
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
    { key: "device_type", label: "Tipo", type: "select", width: 120, editable: true, sortable: true, options: [
      { label: "Cliente", value: "client" },
      { label: "Admin", value: "admin" },
      { label: "IA", value: "client-ia" },
    ]},
  ],
  views: [
    {
      id: "all", label: "Todos", filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["name", "person", "role", "device_type", "active"],
    },
    {
      id: "active", label: "Activos",
      filters: [{ field: "active", op: "eq", value: true }],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["name", "person", "role", "device_type", "active"],
    },
    {
      id: "ia", label: "IA",
      filters: [{ field: "device_type", op: "eq", value: "client-ia" }],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["name", "person", "role", "device_type", "active"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
    ],
  },
  searchFields: ["name", "person", "node_id"],
};
