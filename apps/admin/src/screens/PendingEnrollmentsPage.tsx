import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Button } from "@syntrix/ui/components/ui/button";
import { CheckCircle, XCircle, UserPlus, RefreshCw } from "lucide-react";

type PendingEnrollment = {
  response_id: number;
  peer_id: string;
  node_id: string;
  org_name: string;
  peer: string;
};

export function PendingEnrollmentsPage() {
  const queryClient = useQueryClient();

  const { data: enrollments, isLoading } = useQuery({
    queryKey: ["pending_enrollments"],
    queryFn: async () => await invoke<PendingEnrollment[]>("get_pending_enrollments"),
    refetchInterval: 5000,
  });

  const [approveTarget, setApproveTarget] = useState<PendingEnrollment | null>(null);
  const [selectedRole, setSelectedRole] = useState("sales");
  const [deviceName, setDeviceName] = useState("");
  const [personName, setPersonName] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);

  const handleApprove = async () => {
    if (!approveTarget) return;
    setIsProcessing(true);
    try {
      const orgRoles: { name: string }[] = await invoke("list_roles", { org: approveTarget.org_name });
      const roleToUse = orgRoles.length > 0 ? selectedRole : "sales";
      await invoke("approve_enrollment", {
        org: approveTarget.org_name,
        response_id: approveTarget.response_id,
        role: roleToUse,
        name: deviceName || `Device ${approveTarget.node_id.slice(0, 8)}`,
        person: personName || approveTarget.node_id.slice(0, 8),
      });
      toast.success(`Enrollment approved for ${approveTarget.org_name}`);
      setApproveTarget(null);
      queryClient.invalidateQueries({ queryKey: ["pending_enrollments"] });
    } catch (e: any) {
      toast.error(e || "Error approving enrollment");
    } finally {
      setIsProcessing(false);
    }
  };

  const handleReject = async (enrollment: PendingEnrollment) => {
    try {
      await invoke("reject_enrollment", { response_id: enrollment.response_id, reason: null });
      toast.success("Enrollment rejected");
      queryClient.invalidateQueries({ queryKey: ["pending_enrollments"] });
    } catch (e: any) {
      toast.error(e || "Error rejecting enrollment");
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64 text-muted-foreground">
        <RefreshCw className="h-5 w-5 animate-spin mr-2" />
        Loading pending enrollments...
      </div>
    );
  }

  if (!enrollments || enrollments.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-muted-foreground gap-3">
        <UserPlus className="h-12 w-12 opacity-30" />
        <p className="text-sm">No hay solicitudes de enrolamiento pendientes.</p>
        <p className="text-xs">Las solicitudes aparecen aquí cuando un cliente escanea el código QR de la organización y se conecta.</p>
      </div>
    );
  }

  return (
    <>
      <div className="space-y-3">
        {enrollments.map((e) => (
          <div
            key={e.response_id}
            className="flex items-center justify-between p-4 rounded-lg border bg-card hover:bg-accent/5 transition-colors"
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-1">
                <span className="text-sm font-medium truncate">
                  {e.node_id.slice(0, 16)}...
                </span>
                <span className="text-xs px-2 py-0.5 rounded-full bg-primary/10 text-primary font-medium">
                  {e.org_name}
                </span>
              </div>
              <div className="flex items-center gap-3 text-xs text-muted-foreground">
                <span>Peer: {e.peer.slice(0, 12)}...</span>
                <span>Node: {e.node_id.slice(0, 12)}...</span>
                <span>ID: {e.response_id}</span>
              </div>
            </div>
            <div className="flex items-center gap-2 ml-4 shrink-0">
              <Button
                variant="outline"
                size="sm"
                className="flex items-center gap-1.5"
                onClick={() => {
                  setApproveTarget(e);
                  setSelectedRole("sales");
                  setDeviceName("");
                  setPersonName("");
                }}
              >
                <CheckCircle size={14} className="text-green-600" />
                Approve
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="flex items-center gap-1.5 text-red-600 hover:text-red-700"
                onClick={() => handleReject(e)}
              >
                <XCircle size={14} />
                Reject
              </Button>
            </div>
          </div>
        ))}
      </div>

      {approveTarget && (
        <div
          className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
          onClick={() => setApproveTarget(null)}
        >
          <div
            className="bg-card border border-border rounded-xl shadow-lg max-w-lg w-full p-6 text-card-foreground"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex justify-between items-start mb-4">
              <h3 className="text-lg font-semibold flex items-center gap-2">
                <CheckCircle className="h-5 w-5 text-green-600" />
                Approve Enrollment
              </h3>
              <button onClick={() => setApproveTarget(null)} className="text-muted-foreground hover:text-foreground">
                <XCircle size={18} />
              </button>
            </div>

            <p className="text-sm text-muted-foreground mb-4">
              Approve device <strong>{approveTarget.node_id.slice(0, 16)}...</strong> for org{" "}
              <strong>{approveTarget.org_name}</strong>. The device will receive an invite P2P message.
            </p>

            <div className="space-y-3 mb-6">
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-1 block">
                  Role
                </label>
                <input
                  className="w-full h-10 rounded-md border px-3 py-2 text-sm bg-background"
                  value={selectedRole}
                  onChange={(e) => setSelectedRole(e.target.value)}
                  placeholder="e.g. sales, admin"
                />
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-1 block">
                  Device Name
                </label>
                <input
                  className="w-full h-10 rounded-md border px-3 py-2 text-sm bg-background"
                  value={deviceName}
                  onChange={(e) => setDeviceName(e.target.value)}
                  placeholder={`Device ${approveTarget.node_id.slice(0, 8)}`}
                />
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-1 block">
                  Person
                </label>
                <input
                  className="w-full h-10 rounded-md border px-3 py-2 text-sm bg-background"
                  value={personName}
                  onChange={(e) => setPersonName(e.target.value)}
                  placeholder="Name or identifier"
                />
              </div>
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setApproveTarget(null)} disabled={isProcessing}>
                Cancel
              </Button>
              <Button size="sm" onClick={handleApprove} disabled={isProcessing} className="flex items-center gap-1.5">
                {isProcessing ? (
                  <>
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    Approving...
                  </>
                ) : (
                  <>
                    <CheckCircle size={14} />
                    Approve & Send Invite
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
