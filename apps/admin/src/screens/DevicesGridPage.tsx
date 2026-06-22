import { useMemo } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { devicesEntity } from "../entities/devices";
import { createDevicesCollection } from "../collections/devices";
import { getOrgDb } from "../collections/dbManager";

export function DevicesGridPage({ org }: { org: string }) {
  const collection = useMemo(() => getOrgDb(org).getCollection("devices", () => createDevicesCollection(org)), [org]);
  const entity = useMemo(() => ({ ...devicesEntity, collection }), [collection]);

  return <EntityGrid entity={entity} role="admin" />;
}
