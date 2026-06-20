import { z } from "zod";

export const deviceSchema = z.object({
  node_id: z.string(),
  name: z.string().min(1, "Requerido"),
  person: z.string().min(1, "Requerido"),
  role: z.enum(["admin", "sales", "contabilidad", "hr"]),
  active: z.boolean().default(true),
});

export const roleSchema = z.object({
  name: z.string(),
  can_open: z.array(z.string()).default([]),
  can_write: z.array(z.string()).default([]),
});

export const orgSchema = z.object({
  name: z.string(),
  node_count: z.number().default(0),
  created_at: z.string().default(() => new Date().toISOString()),
});
