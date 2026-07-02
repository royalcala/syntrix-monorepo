// Generates schema.json from shared/drizzle/entity-schema-meta.mjs for Rust consumption.
// Written into crates/syntrix-network (single canonical copy; admin replicates the identical
// relational shape via CDC, so it reuses the same column registry, Fase 4). Run via
// `pnpm export-schema` (see apps/admin/package.json / justfile `drizzle-gen`). Writing this
// from both apps is intentionally idempotent (identical content) to keep `just drizzle-gen`
// order-independent.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { entitySchemas } from "../../../shared/drizzle/entity-schema-meta.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outPath = join(__dirname, "../../../crates/syntrix-network/schema.json");

writeFileSync(outPath, JSON.stringify(entitySchemas, null, 2) + "\n", "utf-8");
console.log(`wrote ${outPath}`);
