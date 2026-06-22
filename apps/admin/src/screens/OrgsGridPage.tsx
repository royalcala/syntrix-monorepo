import { useMemo } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { orgsEntity } from "../entities/orgs";
import { createOrgsCollection } from "../collections/orgs";
import { getOrgDb } from "../collections/dbManager";

export function OrgsGridPage() {
  const collection = useMemo(() => getOrgDb("global").getCollection("orgs", createOrgsCollection), []);
  const entity = useMemo(() => ({ ...orgsEntity, collection }), [collection]);

  return <EntityGrid entity={entity} role="admin" />;
}
