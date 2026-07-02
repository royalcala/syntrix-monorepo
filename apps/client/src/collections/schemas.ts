// Single source of truth for business-entity Zod schemas (Fase 1, tarea 8). Field shapes
// mirror apps/client/drizzle/schema.ts column-for-column (kept in sync by hand: drizzle-zod
// was evaluated but its zod v4 typings conflict with this repo's zod@3.25 pin, so schemas are
// hand-written instead of generated).
//
// The duplicate that used to live in packages/syntrix-ui/src/collections/schemas.ts has been
// removed, along with the dead @tanstack/db adapters that were its only consumers (see
// .kilo/plans/1782949593655-relational-cdc-migration.md, Fase 5).
//
// `id` mirrors the `doc_id` column: `query_entity` (Rust) reconstructs the IPC JSON shape
// with `id` instead of `doc_id` so the frontend field registry keeps working unchanged.
import { z } from "zod";

export const customerSchema = z.object({
  id: z.string(),
  name: z.string(),
  tax_id: z.string().optional(),
  address: z.string().optional(),
  phone: z.string().optional(),
  email: z.string().email().optional(),
});

export const supplierSchema = z.object({
  id: z.string(),
  name: z.string(),
  tax_id: z.string().optional(),
  address: z.string().optional(),
  phone: z.string().optional(),
  email: z.string().email().optional(),
});

export const productSchema = z.object({
  id: z.string(),
  name: z.string(),
  sku: z.string().optional(),
  price: z.number().positive(),
  unit: z.string().default("pza"),
  category: z.string().default("general"),
});

export const invoiceItemSchema = z.object({
  line_id: z.string(),
  product_id: z.string(),
  qty: z.number().positive(),
  price: z.number().positive(),
});

export const invoiceSchema = z.object({
  id: z.string(),
  customer_id: z.string(),
  amount: z.number().positive(),
  status: z.enum(["draft", "open", "paid", "cancelled"]),
  tax_rate: z.number().default(0.16),
  date: z.string().transform((s) => new Date(s)),
  items: z.array(invoiceItemSchema).default([]),
});

export const orderItemSchema = z.object({
  line_id: z.string(),
  product_id: z.string(),
  qty: z.number().positive(),
});

export const orderSchema = z.object({
  id: z.string(),
  customer_id: z.string(),
  status: z.enum(["pending", "confirmed", "shipped", "delivered", "cancelled"]),
  date: z.string().transform((s) => new Date(s)),
  items: z.array(orderItemSchema).default([]),
});

export const payrollSchema = z.object({
  id: z.string(),
  employee_id: z.string(),
  employee_name: z.string(),
  period: z.string(),
  gross_amount: z.number().positive(),
  deductions: z.number().default(0),
  net_amount: z.number().positive(),
  status: z.string().default("draft"),
});

export const devSchema = z.object({
  id: z.string(),
  settings: z.record(z.unknown()).default({}),
});
