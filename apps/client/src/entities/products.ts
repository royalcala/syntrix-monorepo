import type { EntityDefinition } from "../fields/registry";
import { fetchEntityData } from "../collections/adapter";

export const productsEntity: EntityDefinition = {
  id: "products",
  label: "Productos",
  icon: "package",
  loadData: () => fetchEntityData("products"),
  fields: [
    { key: "id", label: "ID", type: "text", width: 100, editable: false, sortable: true },
    { key: "name", label: "Nombre", type: "text", width: 200, editable: true, sortable: true },
    { key: "sku", label: "SKU", type: "text", width: 120, editable: true, sortable: true },
    { key: "price", label: "Precio", type: "currency", width: 120, editable: true, sortable: true },
    { key: "unit", label: "Unidad", type: "text", width: 80, editable: true, sortable: false },
    { key: "category", label: "Categoría", type: "text", width: 150, editable: true, sortable: true },
  ],
  views: [
    {
      id: "all", label: "Todos", filters: [],
      sort: [{ field: "name", dir: "asc" }],
      visibleColumns: ["id", "name", "sku", "price", "unit", "category"],
    },
  ],
  detail: {
    tabs: [
      { key: "data", label: "Datos" },
      { key: "history", label: "Historial" },
    ],
  },
  searchFields: ["name", "sku"],
};
