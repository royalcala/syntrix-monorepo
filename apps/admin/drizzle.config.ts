import { defineConfig } from "drizzle-kit";
export default defineConfig({
  schema: "./drizzle/schema.ts",
  out: "../admin/src-tauri/migrations",
  dialect: "sqlite",
});
