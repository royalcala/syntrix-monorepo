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
    onSaveCreate={async (data) => {
      if (!data.node_id) throw new Error("Debes pegar el Device Address (JSON) en el campo Node ID");
      await invoke("send_invite", {
        org, 
        endpointAddrJson: String(data.node_id), 
        role: String(data.role || "sales"),
        name: String(data.name || "Nuevo Dispositivo"),
        person: String(data.person || "")
      });
      queryClient.invalidateQueries({ queryKey: ["entity", "devices", org] });
      return data as any;
    }}
    customActions={(row) => (
      <button 
        className="px-3 py-1 bg-primary text-primary-foreground text-xs rounded hover:opacity-90 transition-opacity"
        onClick={async () => {
          try {
            const addr = prompt("Pega el Device Address (JSON) que el cliente copió para reenviar la invitación:", "");
            if (!addr) return;
            await invoke("send_invite", {
              org, 
              endpointAddrJson: addr, 
              role: String(row.role || "sales"),
              name: String(row.name || ""),
              person: String(row.person || "")
            });
            alert("Invitación enviada");
          } catch (e: any) {
            alert(e.message || "Error al enviar");
          }
        }}
      >
        Re-enviar Invitación
      </button>
    )}
  />;
}
