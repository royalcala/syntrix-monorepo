import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { rolesEntity } from "../entities/roles";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";

export function RolesGridPage({ org }: { org: string }) {
  const queryClient = useQueryClient();
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
  }, [org]);

  return <EntityGrid 
    entity={rolesEntity} 
    role="admin" 
    orgId={org} 
    onSaveCreate={async (data) => {
      await invoke("create_role", {
        org, name: data.name, canOpen: data.can_open || [], canWrite: data.can_write || []
      });
      queryClient.invalidateQueries({ queryKey: ["entity", "roles", org] });
      return data as any;
    }}
    onSaveUpdate={async (id, data) => {
      await invoke("update_role", {
        org, key: id, changes: data
      });
      queryClient.invalidateQueries({ queryKey: ["entity", "roles", org] });
    }}
  />;
}
