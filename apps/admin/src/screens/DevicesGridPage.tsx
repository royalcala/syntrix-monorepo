import { useState, useEffect, useMemo } from "react";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { devicesEntity } from "../entities/devices";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { Button } from "@syntrix/ui/components/ui/button";
import { RefreshCw, Send, X, Bot } from "lucide-react";

export function DevicesGridPage({ org }: { org: string }) {
  const queryClient = useQueryClient();
  
  // Fetch actual roles
  const { data: roles } = useQuery({
    queryKey: ["entity", "roles", org],
    queryFn: async () => {
      return await invoke<any[]>("list_roles", { org });
    }
  });

  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
  }, [org]);

  // Modal State for Resend
  const [resendRow, setResendRow] = useState<any | null>(null);
  const [deviceAddr, setDeviceAddr] = useState("");
  const [isSending, setIsSending] = useState(false);

  // Inject roles into the entity config dynamically
  const entityWithDynamicRoles = useMemo(() => {
    const newEntity = { ...devicesEntity };
    const roleFieldIdx = newEntity.fields.findIndex(f => f.key === "role");
    if (roleFieldIdx !== -1 && roles && roles.length > 0) {
      newEntity.fields = [...newEntity.fields];
      newEntity.fields[roleFieldIdx] = {
        ...newEntity.fields[roleFieldIdx],
        options: roles.map(r => ({ label: r.name, value: r.name }))
      };
    }
    return newEntity;
  }, [roles]);

  const handleResend = async () => {
    if (!resendRow) return;
    setIsSending(true);
    try {
      await invoke("send_invite", {
        org, 
        endpointAddrJson: deviceAddr, 
        role: String(resendRow.role || "sales"),
        name: String(resendRow.name || ""),
        person: String(resendRow.person || "")
      });
      toast.success("Invitación reenviada correctamente");
      setResendRow(null);
    } catch (e: any) {
      toast.error(e || "Error al enviar invitación");
    } finally {
      setIsSending(false);
    }
  };

  return (
    <>
      <EntityGrid 
        entity={entityWithDynamicRoles} 
        role="admin" 
        orgId={org} 
        onSaveUpdate={async (id, data) => {
          await invoke("update_device", {
            org, nodeId: id, active: Boolean(data.active),
            role: data.role || null, name: data.name || null, person: data.person || null,
            device_type: data.device_type || null,
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
          toast.success("Invitación P2P enviada");
          return data as any;
        }}
        customActions={(row: any) => (
          <div className="flex gap-1">
            {row.device_type !== "client-ia" && (
              <Button
                variant="outline"
                size="sm"
                className="flex items-center gap-1.5"
                onClick={async () => {
                  try {
                    await invoke("update_device", {
                      org, nodeId: row.node_id, active: true, role: null,
                      name: null, person: null, device_type: "client-ia"
                    });
                    queryClient.invalidateQueries({ queryKey: ["entity", "devices", org] });
                    toast.success("Dispositivo designado como IA de la org");
                  } catch (e: any) {
                    toast.error(e || "Error al designar IA");
                  }
                }}
              >
                <Bot size={13} />
                Designar IA
              </Button>
            )}
            <Button
              variant="outline"
              size="sm"
              className="flex items-center gap-1.5"
              onClick={() => {
                setResendRow(row);
                setDeviceAddr(row.device_addr || "");
              }}
            >
              <RefreshCw size={13} />
              Re-enviar
            </Button>
          </div>
        )}
      />

      {resendRow && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={() => setResendRow(null)}>
          <div className="bg-card border border-border rounded-xl shadow-lg max-w-lg w-full p-6 text-card-foreground" onClick={(e) => e.stopPropagation()}>
            <div className="flex justify-between items-start mb-2">
              <h3 className="text-lg font-semibold flex items-center gap-2">
                <RefreshCw className="h-5 w-5 text-primary" />
                Reenviar Invitación P2P
              </h3>
              <button onClick={() => setResendRow(null)} className="text-muted-foreground hover:text-foreground">
                <X size={18} />
              </button>
            </div>
            
            <p className="text-sm text-muted-foreground mb-4">
              Se enviará la invitación a <strong>{resendRow.name || resendRow.person || "este dispositivo"}</strong>. 
              Si el dispositivo cambió de red, puedes actualizar su dirección JSON abajo.
            </p>

            <div className="space-y-2 mb-6">
              <label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Device Address (JSON)
              </label>
              <textarea
                value={deviceAddr}
                onChange={(e) => setDeviceAddr(e.target.value)}
                placeholder='{"node_id": "...", "addrs": ["..."]}'
                className="w-full min-h-[100px] text-xs font-mono p-3 bg-muted border border-input rounded-lg focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              />
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setResendRow(null)} disabled={isSending}>
                Cancelar
              </Button>
              <Button size="sm" onClick={handleResend} disabled={isSending} className="flex items-center gap-1.5">
                {isSending ? (
                  <>
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    Enviando...
                  </>
                ) : (
                  <>
                    <Send size={14} />
                    Enviar
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
