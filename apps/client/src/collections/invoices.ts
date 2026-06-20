import { createCollection } from "@tanstack/react-db";
import { irohCollectionOptions } from "./adapter";
import { invoiceSchema } from "./schemas";

export const invoicesCollection = createCollection(
  irohCollectionOptions({
    dataType: "invoices",
    schema: invoiceSchema,
    getKey: (inv) => inv.id,
    roles: {
      canOpen: ["invoices"],
      canWrite: ["invoices"],
    },
  }),
);
