import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { rolesEntity } from "../entities/roles";

export function RolesGridPage({ org }: { org: string }) {
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
  }, [org]);

  return <EntityGrid entity={rolesEntity} role="admin" orgId={org} />;
}
