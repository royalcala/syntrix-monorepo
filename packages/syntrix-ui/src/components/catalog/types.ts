export interface DataTableColumn {
  key: string;
  label: string;
  sortable?: boolean;
}

export interface DataTableComponent {
  id: string;
  type: "dataTable";
  columns: DataTableColumn[];
  onRowClick?: { entity: string };
}

export interface MetricCardComponent {
  id: string;
  type: "metricCard";
  label: string;
  valueKey: string;
  format?: "currency" | "number" | "percentage";
}

export interface EntityField {
  key: string;
  label: string;
  type: string;
}

export interface EntityRelation {
  entity: string;
  label: string;
  fkField: string;
}

export interface EntityDetailComponent {
  id: string;
  type: "entityDetail";
  entity: string;
  fields: EntityField[];
  relations: EntityRelation[];
}

export interface FormField {
  key: string;
  label: string;
  type: string;
  required?: boolean;
  defaultValue?: unknown;
}

export interface FormValidation {
  field: string;
  op: string;
  value: unknown;
  message: string;
}

export interface FormComponent {
  id: string;
  type: "form";
  entity: string;
  action: "create" | "edit";
  fields: FormField[];
  validations: FormValidation[];
  submitLabel: string;
}

export interface ColumnComponent {
  id: string;
  type: "column";
  children: string[];
}

export interface RowComponent {
  id: string;
  type: "row";
  children: string[];
}

export interface CardComponent {
  id: string;
  type: "card";
  title?: string;
  child: string;
}

export type ComponentDef =
  | DataTableComponent
  | MetricCardComponent
  | EntityDetailComponent
  | FormComponent
  | ColumnComponent
  | RowComponent
  | CardComponent;

export interface ViewMeta {
  naturalLanguageQuery: string;
  createdBy: string;
  createdAt: number;
  lastVisitedAt?: number;
  usageCount: number;
  tags: string[];
}

export interface ViewDefinition {
  id: string;
  orgId: string;
  sql: string;
  entity: string;
  components: ComponentDef[];
  root: string;
  meta: ViewMeta;
}
