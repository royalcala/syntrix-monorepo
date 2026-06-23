import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { devicesEntity } from "../entities/devices";

export function DevicesGridPage({ org }: { org: string }) {
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
  }, [org]);

  return <EntityGrid entity={devicesEntity} role="admin" orgId={org} />;
}
