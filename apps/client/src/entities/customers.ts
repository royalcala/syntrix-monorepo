import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const customersEntity: EntityDefinition = {
  id: "customers",
  label: "Clientes",
  icon: "users",
  loadData: () => fetchEntityData("customers"),
  fields: [
    { key: "id", label: "ID", type: "text", width: 100, editable: false, sortable: true },
    { key: "name", label: "Nombre", type: "text", width: 200, editable: true, sortable: true },
    { key: "tax_id", label: "RFC", type: "text", width: 140, editable: true, sortable: true },
    { key: "email", label: "Email", type: "text", width: 200, editable: true, sortable: true },
    { key: "phone", label: "Teléfono", type: "text", width: 140, editable: true, sortable: false },
    { key: "address", label: "Dirección", type: "text", width: 250, editable: true, sortable: false },
  ],
  views: [
    {
      id: "all",
      label: "Todos",
      filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["id", "name", "tax_id", "email", "phone", "address"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
      { key: "invoices", label: "Facturas" },
      { key: "history", label: "Historial" },
    ],
  },
  searchFields: ["name", "tax_id", "email"],
};
