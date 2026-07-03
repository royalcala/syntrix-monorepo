import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import * as e from "./entities";

export const customersSchemas = {
  insert: createInsertSchema(e.customers),
  select: createSelectSchema(e.customers),
};
export const suppliersSchemas = {
  insert: createInsertSchema(e.suppliers),
  select: createSelectSchema(e.suppliers),
};
export const productsSchemas = {
  insert: createInsertSchema(e.products),
  select: createSelectSchema(e.products),
};
export const invoicesSchemas = {
  insert: createInsertSchema(e.invoices),
  select: createSelectSchema(e.invoices),
};
export const invoiceItemsSchemas = {
  insert: createInsertSchema(e.invoiceItems),
  select: createSelectSchema(e.invoiceItems),
};
export const ordersSchemas = {
  insert: createInsertSchema(e.orders),
  select: createSelectSchema(e.orders),
};
export const orderItemsSchemas = {
  insert: createInsertSchema(e.orderItems),
  select: createSelectSchema(e.orderItems),
};
export const payrollSchemas = {
  insert: createInsertSchema(e.payroll),
  select: createSelectSchema(e.payroll),
};
