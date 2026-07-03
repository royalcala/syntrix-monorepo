import { db } from "../db";
import { eq, and, type SQL } from "drizzle-orm";
import * as schema from "../../../../packages/shared-drizzle/src/entities";

const entityMap: Record<string, { query: any; table: any }> = {
  customers: { query: db.query.customers, table: schema.customers },
  suppliers: { query: db.query.suppliers, table: schema.suppliers },
  products: { query: db.query.products, table: schema.products },
  invoices: { query: db.query.invoices, table: schema.invoices },
  invoiceItems: { query: db.query.invoiceItems, table: schema.invoiceItems },
  orders: { query: db.query.orders, table: schema.orders },
  orderItems: { query: db.query.orderItems, table: schema.orderItems },
  payroll: { query: db.query.payroll, table: schema.payroll },
};

export async function fetchEntityData<T>(
  entityId: string,
  _filterField?: string,
  _filterValue?: string,
  orgId?: string,
): Promise<T[]> {
  try {
    const entry = entityMap[entityId];
    if (!entry) return [];

    const conditions: SQL[] = [];
    if (orgId && entry.table.orgId) {
      conditions.push(eq(entry.table.orgId, orgId));
    }

    const where = conditions.length > 0 ? and(...conditions) : undefined;
    return await entry.query.findMany({ where }) as T[];
  } catch (err) {
    console.error(`Failed to fetch ${entityId}:`, err);
    return [];
  }
}
