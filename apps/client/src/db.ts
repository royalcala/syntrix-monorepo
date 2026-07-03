import { drizzle } from "drizzle-orm/sqlite-proxy";
import { invoke } from "@tauri-apps/api/core";
import * as schema from "../../../packages/shared-drizzle/src/entities";

export const db = drizzle<typeof schema>(
  async (sql, params, _method) => {
    try {
      const result = await invoke<{ rows: unknown[][] }>("drizzle_execute", {
        sql,
        params: params.map(String),
      });
      return { rows: result.rows };
    } catch (e) {
      console.error("Drizzle execute error:", e);
      return { rows: [] };
    }
  },
  { schema, logger: false },
);
