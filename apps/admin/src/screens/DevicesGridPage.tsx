import { useEffect } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { devicesEntity } from "../entities/devices";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";

export function DevicesGridPage({ org }: { org: string }) {
  const queryClient = useQueryClient();
  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
  }, [org]);

  return <EntityGrid 
    entity={devicesEntity} 
    role="admin" 
    orgId={org} 
    onSaveUpdate={async (id, data) => {
      await invoke("update_device", {
        org, nodeId: id, active: Boolean(data.active), 
        role: data.role || null, name: data.name || null, person: data.person || null
      });
      queryClient.invalidateQueries({ queryKey: ["entity", "devices", org] });
    }}
    onSaveCreate={async () => { throw new Error("Add devices via invitations, not here."); }}
  />;
}
