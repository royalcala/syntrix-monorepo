import { useMemo } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { rolesEntity } from "../entities/roles";
import { createRolesCollection } from "../collections/roles";
import { getOrgDb } from "../collections/dbManager";

export function RolesGridPage({ org }: { org: string }) {
  const collection = useMemo(() => getOrgDb(org).getCollection("roles", createRolesCollection), [org]);
  const entity = useMemo(() => ({ ...rolesEntity, collection }), [collection]);

  return <EntityGrid entity={entity} role="admin" />;
}
