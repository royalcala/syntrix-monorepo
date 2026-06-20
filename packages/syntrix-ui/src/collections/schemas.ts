import { z } from "zod";

export const invoiceSchema = z.object({
  id: z.string(),
  amount: z.number().positive(),
  status: z.enum(["draft", "open", "paid", "cancelled"]),
  customer_id: z.string(),
  date: z.string().transform((s) => new Date(s)),
  items: z
    .array(
      z.object({
        product_id: z.string(),
        qty: z.number().positive(),
        price: z.number().positive(),
      }),
    )
    .default([]),
  tax_rate: z.number().default(0.16),
  _namespace: z.string().optional(),
  _author: z.string().optional(),
});

export const productSchema = z.object({
  id: z.string(),
  name: z.string(),
  sku: z.string().optional(),
  price: z.number().positive(),
  unit: z.string().default("pza"),
  category: z.string().default("general"),
  _namespace: z.string().optional(),
  _author: z.string().optional(),
});

export const customerSchema = z.object({
  id: z.string(),
  name: z.string(),
  tax_id: z.string().optional(),
  address: z.string().optional(),
  phone: z.string().optional(),
  email: z.string().email().optional(),
  _namespace: z.string().optional(),
  _author: z.string().optional(),
});

export const orderSchema = z.object({
  id: z.string(),
  customer_id: z.string(),
  items: z.array(
    z.object({
      product_id: z.string(),
      qty: z.number().positive(),
    }),
  ),
  status: z.enum(["pending", "confirmed", "shipped", "delivered", "cancelled"]),
  date: z.string().transform((s) => new Date(s)),
  _namespace: z.string().optional(),
  _author: z.string().optional(),
});

export const devSchema = z.object({
  id: z.string(),
  settings: z.record(z.unknown()).default({}),
});
