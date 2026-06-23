import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { orgsEntity } from "../entities/orgs";

export function OrgsGridPage() {
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", "");
  }, []);

  return <EntityGrid entity={orgsEntity} role="admin" />;
}
