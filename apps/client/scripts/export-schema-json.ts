// Thin wrapper that calls `generateSchemaJson` from @syntrix/shared-drizzle
// and writes the result to `crates/syntrix-network/schema.json`.
import { writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { generateSchemaJson } from "../../../packages/shared-drizzle/src/export-schema";

const outPath = join(dirname(fileURLToPath(import.meta.url)), "../../../crates/syntrix-network/schema.json");
writeFileSync(outPath, JSON.stringify(generateSchemaJson(), null, 2) + "\n", "utf-8");
console.log(`wrote ${outPath} (from Drizzle entities)`);
