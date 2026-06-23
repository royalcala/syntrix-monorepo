import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { orgsEntity } from "../entities/orgs";

export function OrgsGridPage() {
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", "");
  }, []);

  return <EntityGrid 
    entity={orgsEntity} 
    role="admin" 
    onSaveCreate={async () => { throw new Error("No se pueden crear orgs desde aquí"); }}
    onSaveUpdate={async () => { throw new Error("No se pueden editar orgs desde aquí"); }}
  />;
}
