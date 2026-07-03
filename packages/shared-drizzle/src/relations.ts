import { relations } from "drizzle-orm";
import { invoices, invoiceItems, orders, orderItems } from "./entities";

export const invoicesRelations = relations(invoices, ({ many }) => ({
  items: many(invoiceItems),
}));

export const ordersRelations = relations(orders, ({ many }) => ({
  items: many(orderItems),
}));
