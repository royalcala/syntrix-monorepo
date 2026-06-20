import type { ReactNode } from "react";

export interface FieldTypePlugin {
  id: string;
  detail: {
    renderEditor: (value: unknown, row: Record<string, unknown>, onChange: (value: unknown) => void) => ReactNode;
    renderViewer: (value: unknown) => ReactNode;
  };
  validate?: (value: unknown) => string | null;
  sort?: (a: unknown, b: unknown, dir: "asc" | "desc") => number;
  filterOperators: string[];
}

export interface EntityFieldConfig {
  key: string;
  label: string;
  type: string;
  width: number;
  editable: boolean;
  sortable: boolean;
  options?: { label: string; value: string }[];
  theme?: Record<string, unknown>;
  permissions?: { view?: string[]; edit?: string[] };
}

export interface ViewDefinition {
  id: string;
  label: string;
  filters: FilterRule[];
  sort: { field: string; dir: "asc" | "desc" }[];
  visibleColumns: string[];
}

export interface FilterRule {
  field: string;
  op: string;
  value: unknown;
}

export interface DetailTab { key: string; label: string; icon?: string; }

export interface EntityDefinition {
  id: string;
  label: string;
  icon: string;
  collection: unknown;
  fields: EntityFieldConfig[];
  views: ViewDefinition[];
  detail: { tabs: DetailTab[] };
  searchFields: string[];
  canEdit?: (row: Record<string, unknown>, role?: string) => boolean;
  canDelete?: (row: Record<string, unknown>, role?: string) => boolean;
}

export interface EntityAction { id: string; label: string; icon: string; handler: () => void; }

type FieldRenderers = Record<string, FieldTypePlugin>;
const fieldRenderers: FieldRenderers = {};

export function registerFieldType(plugin: FieldTypePlugin): void { fieldRenderers[plugin.id] = plugin; }

export function getFieldRenderer(type: string): FieldTypePlugin {
  return fieldRenderers[type] ?? textField;
}

export const textField: FieldTypePlugin = {
  id: "text",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => String(v ?? ""),
  },
  filterOperators: ["eq", "neq", "contains", "startsWith"],
};

export const numberField: FieldTypePlugin = {
  id: "number",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => String(v ?? 0),
  },
  sort: (a, b, dir) => (dir === "asc" ? Number(a) - Number(b) : Number(b) - Number(a)),
  filterOperators: ["eq", "neq", "gt", "gte", "lt", "lte"],
};

export const currencyField: FieldTypePlugin = {
  id: "currency",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => `$${Number(v ?? 0).toFixed(2)}`,
  },
  sort: (a, b, dir) => (dir === "asc" ? Number(a) - Number(b) : Number(b) - Number(a)),
  filterOperators: ["eq", "gt", "gte", "lt", "lte"],
};

export const dateField: FieldTypePlugin = {
  id: "date",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => (v instanceof Date ? v.toLocaleDateString() : String(v ?? "")),
  },
  sort: (a, b, dir) => {
    const da = new Date(String(a ?? 0)).getTime();
    const db = new Date(String(b ?? 0)).getTime();
    return dir === "asc" ? da - db : db - da;
  },
  filterOperators: ["eq", "gt", "gte", "lt", "lte"],
};

export const selectField: FieldTypePlugin = {
  id: "select",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => String(v ?? ""),
  },
  filterOperators: ["eq", "neq"],
};

export const statusField: FieldTypePlugin = {
  id: "status",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => String(v ?? ""),
  },
  filterOperators: ["eq", "neq"],
};

export const relationField: FieldTypePlugin = {
  id: "relation",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => String(v ?? ""),
  },
  filterOperators: ["eq", "neq"],
};

export const booleanField: FieldTypePlugin = {
  id: "boolean",
  detail: {
    renderEditor: () => null,
    renderViewer: (v) => (v ? "Sí" : "No"),
  },
  filterOperators: ["eq"],
};

registerFieldType(textField);
registerFieldType(numberField);
registerFieldType(currencyField);
registerFieldType(dateField);
registerFieldType(selectField);
registerFieldType(statusField);
registerFieldType(relationField);
registerFieldType(booleanField);
