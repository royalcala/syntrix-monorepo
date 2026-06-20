import { useMemo } from "react";
import { EntityGrid } from "../components/EntityGrid";
import { rolesEntity } from "../entities/roles";
import { createRolesCollection } from "../collections/roles";

export function RolesGridPage({ org }: { org: string }) {
  const collection = useMemo(() => createRolesCollection(org), [org]);
  const entity = useMemo(() => ({ ...rolesEntity, collection }), [collection]);

  return <EntityGrid entity={entity} role="admin" />;
}
