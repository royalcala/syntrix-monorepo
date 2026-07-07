import * as entities from "./entities";

const searchableColumns: Record<string, string[]> = {
  customers: ["name", "tax_id", "email"],
  suppliers: ["name", "tax_id", "email"],
  products:  ["name", "sku", "category"],
  invoices:  ["customer_id", "status"],
  orders:    ["customer_id", "status"],
  payroll:   ["employee_id", "employee_name", "period", "status"],
};

const childTables: Record<string, Record<string, string>> = {
  invoices: { invoice_items: "invoice_id" },
  orders:   { order_items: "order_id" },
};

function getColumns(table: any): Record<string, any> {
  const cols: Record<string, any> = {};
  if (typeof table !== "object" || table === null) return cols;
  for (const key of Object.keys(table)) {
    const val = table[key];
    if (val && typeof val === "object" && "dataType" in val) {
      cols[key] = val;
    }
  }
  return cols;
}

function getSqlType(col: any): string {
  const dt: string = col.dataType ?? "";
  if (dt === "string") return "text";
  if (dt === "number") {
    // For SQLite columns with dataType "number", check if it's integer or real.
    // Drizzle SQLiteInteger and SQLiteReal both have dataType "number".
    const name = col.columnType ?? "";
    if (name.includes("Integer") || name.includes("integer")) return "integer";
    return "real";
  }
  return "text";
}

export function generateSchemaJson(): Record<string, unknown> {
  const schema: Record<string, any> = {};

  // Entity tables and their JS export names (camelCase in TypeScript, snake_case in SQL)
  const entityExports: Record<string, string> = {
    customers: "customers",
    suppliers: "suppliers",
    products: "products",
    invoices: "invoices",
    invoiceItems: "invoice_items",
    orders: "orders",
    orderItems: "order_items",
    payroll: "payroll",
    viewDefinitions: "view_definitions",
    iaQueries: "ia_queries",
  };

  for (const [jsName, tableName] of Object.entries(entityExports)) {
    const table = (entities as any)[jsName];
    if (!table) continue;

    const drizzleColumns = getColumns(table);
    if (Object.keys(drizzleColumns).length === 0) continue;

    const cols: any[] = [];
    for (const [, col] of Object.entries(drizzleColumns)) {
      const colName = (col as any).name ?? "";
      const notNull = (col as any).notNull ?? false;
      const isSyncMeta = ["org_id", "doc_id", "change_time", "node_id"].includes(colName);

      cols.push({
        name: colName,
        type: getSqlType(col as any),
        ...(!notNull && !isSyncMeta ? { nullable: true } : {}),
        ...(searchableColumns[tableName]?.includes(colName) ? { searchable: true } : {}),
      });
    }

    const entry: any = { table: tableName, columns: cols };

    const entityChildren = childTables[tableName];
    if (entityChildren) {
      entry.children = [];
      for (const [childName, parentKey] of Object.entries(entityChildren)) {
        const childJsName = childName === "invoice_items" ? "invoiceItems" : childName === "order_items" ? "orderItems" : childName;
        const childTable = (entities as any)[childJsName];
        if (!childTable) continue;

        const childDrizzleColumns = getColumns(childTable);
        const childCols: any[] = [];
        for (const [, col] of Object.entries(childDrizzleColumns)) {
          const childColName = (col as any).name ?? "";
          const childNotNull = (col as any).notNull ?? false;
          const childIsMeta = ["org_id", "change_time", "node_id"].includes(childColName);
          childCols.push({
            name: childColName,
            type: getSqlType(col as any),
            ...(!childNotNull && !childIsMeta ? { nullable: true } : {}),
          });
        }
        entry.children.push({ table: childName, parentKey, columns: childCols });
      }
    }

    schema[tableName] = entry;
  }

  return schema;
}
