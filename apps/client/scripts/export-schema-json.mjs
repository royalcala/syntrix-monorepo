// Generates schema.json from shared/drizzle/entity-schema-meta.mjs for Rust consumption.
// Written into crates/syntrix-network (lives there instead of syntrix-core to avoid a
// circular crate dependency: syntrix-core already depends on syntrix-network, and
// syntrix-network::cdc needs the column registry too; syntrix-core re-exports it as
// `syntrix_core::schema`). Run via `pnpm export-schema` (see apps/client/package.json /
// justfile `drizzle-gen`).
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { entitySchemas } from "../../../shared/drizzle/entity-schema-meta.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outPath = join(__dirname, "../../../crates/syntrix-network/schema.json");

writeFileSync(outPath, JSON.stringify(entitySchemas, null, 2) + "\n", "utf-8");
console.log(`wrote ${outPath}`);
