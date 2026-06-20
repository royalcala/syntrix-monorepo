import { createCollection } from "@tanstack/react-db";
import { irohCollectionOptions } from "./adapter";
import { productSchema } from "./schemas";

export const productsCollection = createCollection(
  irohCollectionOptions({
    dataType: "products",
    schema: productSchema,
    getKey: (p) => p.id,
    roles: {
      canOpen: ["products"],
      canWrite: ["products"],
    },
  }),
);
