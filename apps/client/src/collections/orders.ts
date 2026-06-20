import { createCollection } from "@tanstack/react-db";
import { irohCollectionOptions } from "./adapter";
import { orderSchema } from "./schemas";

export const ordersCollection = createCollection(
  irohCollectionOptions({
    dataType: "orders",
    schema: orderSchema,
    getKey: (o) => o.id,
    roles: {
      canOpen: ["orders"],
      canWrite: ["orders"],
    },
  }),
);
