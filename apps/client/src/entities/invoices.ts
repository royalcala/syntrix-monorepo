import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const invoicesEntity: EntityDefinition = {
  id: "invoices",
  label: "Facturas",
  icon: "receipt",
  loadData: (orgId?: string) => fetchEntityData("invoices", undefined, undefined, orgId),
  fields: [
    { key: "id", label: "Folio", type: "text", width: 110, editable: false, sortable: true },
    { key: "customer_id", label: "Cliente", type: "relation", width: 200, editable: true, sortable: true },
    { key: "date", label: "Fecha", type: "date", width: 120, editable: true, sortable: true },
    { key: "amount", label: "Total", type: "currency", width: 120, editable: false, sortable: true },
    { key: "status", label: "Estado", type: "status", width: 110, editable: true, sortable: true, options: [
      { label: "Borrador", value: "draft" },
      { label: "Abierta", value: "open" },
      { label: "Pagada", value: "paid" },
      { label: "Cancelada", value: "cancelled" },
    ]},
  ],
  views: [
    {
      id: "all", label: "Todas", filters: [],
      sort: [{ field: "date", dir: "desc" }],
      visibleColumns: ["id", "customer_id", "date", "amount", "status"],
    },
    {
      id: "open", label: "Pendientes",
      filters: [{ field: "status", op: "eq", value: "open" }],
      sort: [{ field: "date", dir: "desc" }],
      visibleColumns: ["id", "customer_id", "date", "amount", "status"],
    },
    {
      id: "paid", label: "Pagadas",
      filters: [{ field: "status", op: "eq", value: "paid" }],
      sort: [{ field: "date", dir: "desc" }],
      visibleColumns: ["id", "customer_id", "date", "amount", "status"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
      { key: "items", label: "Items" },
      { key: "history", label: "Historial" },
    ],
  },
  searchFields: ["id", "customer_id"],
};
