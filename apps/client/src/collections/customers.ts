import { createCollection } from "@tanstack/react-db";
import { irohCollectionOptions } from "./adapter";
import { customerSchema } from "./schemas";

export const customersCollection = createCollection(
  irohCollectionOptions({
    dataType: "customers",
    schema: customerSchema,
    getKey: (c) => c.id,
    roles: {
      canOpen: ["customers"],
      canWrite: ["customers"],
    },
  }),
);
