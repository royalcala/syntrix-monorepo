import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const rolesEntity: EntityDefinition = {
  id: "roles",
  label: "Roles",
  icon: "shield",
  loadData: (orgId?: string) => fetchEntityData("roles", undefined, undefined, orgId),
  fields: [
    { key: "name", label: "Nombre", type: "text", width: 180, editable: false, sortable: true },
    { 
      key: "can_open", 
      label: "Leer", 
      type: "multi-select", 
      width: 200, 
      editable: true, 
      sortable: false,
      options: [
        { label: "Clientes", value: "customers" },
        { label: "Proveedores", value: "suppliers" },
        { label: "Productos", value: "products" },
        { label: "Facturas", value: "invoices" },
        { label: "Órdenes", value: "orders" },
        { label: "Nómina", value: "payroll" }
      ]
    },
    { 
      key: "can_write", 
      label: "Escribir", 
      type: "multi-select", 
      width: 200, 
      editable: true, 
      sortable: false,
      options: [
        { label: "Clientes", value: "customers" },
        { label: "Proveedores", value: "suppliers" },
        { label: "Productos", value: "products" },
        { label: "Facturas", value: "invoices" },
        { label: "Órdenes", value: "orders" },
        { label: "Nómina", value: "payroll" }
      ]
    },
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
