import { useMemo } from "react";
import { EntityGrid } from "../components/EntityGrid";
import { orgsEntity } from "../entities/orgs";
import { createOrgsCollection } from "../collections/orgs";

export function OrgsGridPage() {
  const collection = useMemo(() => createOrgsCollection(), []);
  const entity = useMemo(() => ({ ...orgsEntity, collection }), [collection]);

  return <EntityGrid entity={entity} role="admin" />;
}
