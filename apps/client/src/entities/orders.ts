import type { EntityDefinition } from "../fields/registry";
import { ordersCollection } from "../collections/orders";

export const ordersEntity: EntityDefinition = {
  id: "orders",
  label: "Órdenes",
  icon: "shopping-cart",
  collection: ordersCollection as never,
  fields: [
    { key: "id", label: "ID", type: "text", width: 100, editable: false, sortable: true },
    { key: "customer_id", label: "Cliente", type: "relation", width: 200, editable: true, sortable: true },
    { key: "date", label: "Fecha", type: "date", width: 120, editable: true, sortable: true },
    { key: "status", label: "Estado", type: "status", width: 110, editable: true, sortable: true, options: [
      { label: "Pendiente", value: "pending" },
      { label: "Confirmada", value: "confirmed" },
      { label: "Enviada", value: "shipped" },
      { label: "Entregada", value: "delivered" },
      { label: "Cancelada", value: "cancelled" },
    ]},
  ],
  views: [
    {
      id: "all", label: "Todas", filters: [],
      sort: [{ field: "date", dir: "desc" }],
      visibleColumns: ["id", "customer_id", "date", "status"],
    },
    {
      id: "pending", label: "Pendientes",
      filters: [{ field: "status", op: "eq", value: "pending" }],
      sort: [{ field: "date", dir: "desc" }],
      visibleColumns: ["id", "customer_id", "date", "status"],
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
