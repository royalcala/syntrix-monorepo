import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Activity, CheckCircle2, XCircle } from "lucide-react";
import { Button } from "./ui/button";

export interface PeerStatus {
  node_id: string;
  status: string;
  last_seen: number;
}

export interface SyncInfo {
  node_id: string;
  is_online: boolean;
  peers: PeerStatus[];
}

export function SyncStatusIndicator({ org }: { org: string }) {
  const [info, setInfo] = useState<SyncInfo | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    if (!org) return;
    
    // Poll every 5 seconds
    const fetchInfo = async () => {
      try {
        const data = await invoke<SyncInfo>("get_sync_info", { org });
        setInfo(data);
      } catch (e) {
        console.error("Failed to get sync info", e);
      }
    };
    
    fetchInfo();
    const intv = setInterval(fetchInfo, 5000);
    return () => clearInterval(intv);
  }, [org]);

  if (!info) return null;

  const onlinePeers = info.peers.filter(p => p.status === "online").length;
  const totalPeers = info.peers.length;
  const isHealthy = onlinePeers > 0 || totalPeers === 0;

  return (
    <div className="p-3 bg-card border rounded-lg shadow-sm text-sm space-y-2">
      <div className="flex items-center gap-2 font-medium">
        {isHealthy ? (
          <CheckCircle2 size={16} className="text-emerald-500" />
        ) : (
          <XCircle size={16} className="text-amber-500" />
        )}
        <span>{isHealthy ? "Sincronizado" : "Revisando P2P"}</span>
      </div>
      
      <div className="text-xs text-muted-foreground flex items-center justify-between">
        <span>Conectado: {info.is_online ? "Sí" : "No"}</span>
        <span className="flex items-center gap-1">
          <Activity size={12} className={onlinePeers > 0 ? "text-emerald-500" : "text-muted-foreground"} />
          {onlinePeers}/{totalPeers} Peers
        </span>
      </div>
      
      {info.peers.length > 0 && (
        <div className="pt-2 mt-2 border-t text-xs space-y-1">
          {info.peers.slice(0, 3).map(p => {
            const isOnline = p.status === "online";
            return (
              <div key={p.node_id} className="flex items-center justify-between">
                <span className="truncate max-w-[100px]" title={p.node_id}>{p.node_id.slice(0, 8)}...</span>
                <span className="flex items-center gap-1">
                  <span className={`w-1.5 h-1.5 rounded-full ${isOnline ? 'bg-emerald-500' : 'bg-red-500'}`} />
                  {isOnline ? "On" : "Off"}
                </span>
              </div>
            );
          })}
          {info.peers.length > 3 && <div className="text-center text-[10px] text-muted-foreground pt-1">+{info.peers.length - 3} más</div>}
        </div>
      )}
      
      <Button 
        variant="ghost" 
        size="sm" 
        className="w-full text-xs h-7 text-primary hover:text-primary hover:bg-primary/5 border border-dashed border-primary/20 hover:border-primary/30 mt-2 font-normal rounded-md"
        onClick={() => navigate("/sync")}
      >
        Detalles de red
      </Button>
    </div>
  );
}
