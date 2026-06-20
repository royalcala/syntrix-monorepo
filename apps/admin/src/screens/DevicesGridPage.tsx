import { useState, useMemo } from "react";
import { EntityGrid } from "../components/EntityGrid";
import { devicesEntity } from "../entities/devices";
import { createDevicesCollection } from "../collections/devices";

export function DevicesGridPage({ org }: { org: string }) {
  const [tick, setTick] = useState(0);
  const collection = useMemo(() => createDevicesCollection(org), [org, tick]);
  const entity = useMemo(() => ({ ...devicesEntity, collection }), [collection]);

  return (
    <EntityGrid
      key={tick}
      entity={entity}
      role="admin"
      onSaveCreate={() => {
        // Collection handles the insert via adapter's onInsert → invoke("add_device")
        // After create, remount to reload
        setTick((t) => t + 1);
        return Promise.resolve({ id: "" });
      }}
    />
  );
}
