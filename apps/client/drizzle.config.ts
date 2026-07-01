import { defineConfig } from "drizzle-kit";
export default defineConfig({
  schema: "./drizzle/schema.ts",
  out: "../client/src-tauri/migrations",
  dialect: "sqlite",
});
