import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { 
  Activity, 
  Copy, 
  Check, 
  Laptop, 
  Network, 
  Shield, 
  User, 
  MapPin, 
  RefreshCw,
  Database,
  Clock,
} from "lucide-react";
import { PageLayout } from "./PageLayout";
import { Button } from "./ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "./ui/card";
import { Badge } from "./ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";

export interface PeerStatus {
  node_id: string;
  status: string;
  last_seen: number;
  name?: string;
  person?: string;
  role?: string;
  device_addr?: string;
}

export interface SyncInfo {
  node_id: string;
  is_online: boolean;
  peers: PeerStatus[];
}

interface SyncDetailsPageProps {
  org: string;
  nodeId?: string;
}

export function SyncDetailsPage({ org, nodeId: propNodeId }: SyncDetailsPageProps) {
  const [info, setInfo] = useState<SyncInfo | null>(null);
  const [localAddr, setLocalAddr] = useState<{ node_id: string; addrs: string[] } | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [copiedAddr, setCopiedAddr] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const fetchSyncInfo = async (showRefreshIndicator = false) => {
    if (!org) return;
    if (showRefreshIndicator) setRefreshing(true);
    try {
      const data = await invoke<SyncInfo>("get_sync_info", { org });
      setInfo(data);
      
      // Also get local endpoint addresses
      const addrStr = await invoke<string>("get_endpoint_addr");
      if (addrStr) {
        setLocalAddr(JSON.parse(addrStr));
      }
    } catch (e) {
      console.error("Failed to fetch sync details:", e);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    fetchSyncInfo();
    const interval = setInterval(() => fetchSyncInfo(), 5000);
    return () => clearInterval(interval);
  }, [org]);

  const copyToClipboard = async (text: string, type: "id" | "addr", key: string) => {
    try {
      if (navigator.clipboard && window.isSecureContext) {
        await navigator.clipboard.writeText(text);
      } else {
        const textArea = document.createElement("textarea");
        textArea.value = text;
        textArea.style.position = "absolute";
        textArea.style.left = "-9999px";
        document.body.appendChild(textArea);
        textArea.select();
        document.execCommand("copy");
        textArea.remove();
      }
      if (type === "id") {
        setCopiedId(key);
        setTimeout(() => setCopiedId(null), 2000);
      } else {
        setCopiedAddr(key);
        setTimeout(() => setCopiedAddr(null), 2000);
      }
    } catch (err) {
      console.error("Failed to copy text: ", err);
    }
  };

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center min-h-[400px] gap-3">
        <RefreshCw className="animate-spin text-primary" size={32} />
        <p className="text-sm text-muted-foreground">Cargando estado de la red P2P...</p>
      </div>
    );
  }

  const onlinePeers = info?.peers.filter((p) => p.status === "online").length ?? 0;
  const totalPeers = info?.peers.length ?? 0;
  const isHealthy = onlinePeers > 0 || totalPeers === 0;

  const getRelativeTime = (ts: number) => {
    if (!ts || ts === 0) return "Nunca";
    const diff = Date.now() - ts;
    const seconds = Math.floor(diff / 1000);
    if (seconds < 10) return "Hace un momento";
    if (seconds < 60) return `Hace ${seconds} segundos`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `Hace ${minutes}m`;
    const hours = Math.floor(minutes / 60);
    return `Hace ${hours}h`;
  };

  return (
    <PageLayout
      title={
        <div className="flex items-center gap-2">
          <Network size={22} className="text-primary" />
          <span>Sincronización P2P</span>
          {refreshing && <RefreshCw size={14} className="animate-spin text-muted-foreground ml-2" />}
        </div>
      }
      description={
        <>
          Estado de sincronización y dispositivos conectados para la organización:{" "}
          <span className="font-semibold text-foreground">{org}</span>
        </>
      }
      actions={
        <Button variant="outline" size="sm" onClick={() => fetchSyncInfo(true)} className="flex items-center gap-1.5">
          <RefreshCw size={14} className={refreshing ? "animate-spin" : ""} />
          Actualizar
        </Button>
      }
    >

      {/* CDC Health metrics */}
      {(() => {
        const peersWithFreshness = info?.peers.map(p => ({
          ...p,
          latency: p.last_seen ? Date.now() - p.last_seen : null,
        })) || [];
        const avgLatency = peersWithFreshness
          .filter(p => p.latency !== null && p.status === "online")
          .reduce((sum, p, _, arr) => sum + (p.latency || 0) / arr.length, 0);
        const isFresh = avgLatency < 30000; // less than 30s = fresh
        return (
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
            <Card className="backdrop-blur-md bg-card/90 border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1">
                  <Database size={14} />
                  Réplica CDC
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-2">
                  <span className={`w-2 h-2 rounded-full ${isFresh ? 'bg-emerald-500' : 'bg-amber-500'}`} />
                  <span className="text-xs">{isFresh ? "Al día" : "Atrasada"}</span>
                </div>
              </CardContent>
            </Card>

            <Card className="backdrop-blur-md bg-card/90 border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1">
                  <Activity size={14} />
                  Pares Online
                </CardTitle>
              </CardHeader>
              <CardContent>
                <span className="text-2xl font-bold">{onlinePeers}</span>
                <span className="text-xs text-muted-foreground ml-1">/ {totalPeers}</span>
              </CardContent>
            </Card>

            <Card className="backdrop-blur-md bg-card/90 border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1">
                  <Clock size={14} />
                  Latencia Estimada
                </CardTitle>
              </CardHeader>
              <CardContent>
                <span className="text-lg font-bold">
                  {avgLatency > 0 ? `${Math.round(avgLatency / 1000)}s` : "—"}
                </span>
              </CardContent>
            </Card>

            <Card className="backdrop-blur-md bg-card/90 border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1">
                  <Network size={14} />
                  Estado
                </CardTitle>
              </CardHeader>
              <CardContent>
                <Badge className={isHealthy ? "bg-emerald-500/10 text-emerald-500" : "bg-amber-500/10 text-amber-500"}>
                  {isHealthy ? "Sincronizado" : "Desconectado"}
                </Badge>
              </CardContent>
            </Card>
          </div>
        );
      })()}

      {/* Local Node Info */}
      <div className="mb-6">
        <Card className="backdrop-blur-md bg-card/90 border-border">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-2">
              <Laptop size={16} />
              Este Dispositivo (Local)
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1">
              <span className="text-xs text-muted-foreground font-medium block">Node ID del Dispositivo</span>
              <div className="flex items-center gap-2 bg-accent/30 p-2 rounded-md border border-border/50">
                <code className="text-xs font-mono break-all flex-1 select-all">
                  {localAddr?.node_id ?? propNodeId ?? info?.node_id ?? "Cargando ID..."}
                </code>
                <Button 
                  variant="ghost" 
                  size="icon" 
                  className="h-7 w-7 text-muted-foreground"
                  onClick={() => copyToClipboard(localAddr?.node_id ?? propNodeId ?? info?.node_id ?? "", "id", "local")}
                >
                  {copiedId === "local" ? <Check size={14} className="text-emerald-500" /> : <Copy size={14} />}
                </Button>
              </div>
            </div>

            {localAddr && localAddr.addrs && localAddr.addrs.length > 0 && (
              <div className="space-y-1">
                <span className="text-xs text-muted-foreground font-medium block flex items-center gap-1">
                  <MapPin size={12} />
                  Direcciones de Escucha Activas
                </span>
                <div className="max-h-24 overflow-y-auto space-y-1 pr-1">
                  {localAddr.addrs.map((addr, idx) => (
                    <div key={idx} className="flex items-center justify-between text-xs font-mono bg-accent/10 p-1 px-2 rounded border border-border/20">
                      <span className="truncate">{addr}</span>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-5 w-5 text-muted-foreground"
                        onClick={() => copyToClipboard(addr, "addr", `local-addr-${idx}`)}
                      >
                        {copiedAddr === `local-addr-${idx}` ? <Check size={10} className="text-emerald-500" /> : <Copy size={10} />}
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Peers List Table */}
      <Card className="backdrop-blur-md bg-card/90 border-border">
        <CardHeader>
          <div>
            <CardTitle>Dispositivos de la Organización</CardTitle>
            <p className="text-sm text-muted-foreground mt-1">
              Lista de todos los dispositivos autorizados para sincronizar datos dentro de esta organización.
            </p>
          </div>
        </CardHeader>
        <CardContent>
          {totalPeers === 0 ? (
            <div className="text-center py-12 space-y-3">
              <Network className="mx-auto text-muted-foreground opacity-40" size={40} />
              <p className="text-sm text-muted-foreground">No hay dispositivos registrados.</p>
              <p className="text-xs text-muted-foreground">Agrega un dispositivo en la sección de administración o genera un ticket de invitación.</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Dispositivo</TableHead>
                    <TableHead>Usuario / Persona</TableHead>
                    <TableHead>Rol</TableHead>
                    <TableHead>Node ID</TableHead>
                    <TableHead>Estado</TableHead>
                    <TableHead>Última Actividad</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {info?.peers.map((peer) => {
                    const isLocal = peer.node_id === (localAddr?.node_id ?? propNodeId ?? info?.node_id);
                    const isOnline = peer.status === "online";
                    
                    return (
                      <TableRow key={peer.node_id} className={isLocal ? "bg-primary/5 hover:bg-primary/10" : ""}>
                        <TableCell className="font-medium">
                          <div className="flex items-center gap-2">
                            <Laptop size={16} className="text-muted-foreground" />
                            <span>
                              {peer.name || (isLocal ? "Este Dispositivo (Admin)" : "Dispositivo sin nombre")}
                              {isLocal && <span className="text-[10px] text-primary bg-primary/10 border border-primary/20 p-0.5 px-1 rounded ml-1.5 font-normal">Local</span>}
                            </span>
                          </div>
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-1.5 text-sm text-muted-foreground">
                            <User size={14} />
                            <span>{peer.person || "N/A"}</span>
                          </div>
                        </TableCell>
                        <TableCell>
                          {peer.role ? (
                            <Badge variant="outline" className="flex items-center gap-1 w-fit capitalize font-normal border-primary/20">
                              <Shield size={12} className="text-primary" />
                              {peer.role}
                            </Badge>
                          ) : (
                            <span className="text-xs text-muted-foreground">-</span>
                          )}
                        </TableCell>
                        <TableCell className="font-mono text-xs max-w-[120px]">
                          <div className="flex items-center gap-1.5">
                            <span className="truncate" title={peer.node_id}>
                              {peer.node_id.slice(0, 16)}...
                            </span>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-6 w-6 text-muted-foreground"
                              onClick={() => copyToClipboard(peer.node_id, "id", peer.node_id)}
                            >
                              {copiedId === peer.node_id ? <Check size={12} className="text-emerald-500" /> : <Copy size={12} />}
                            </Button>
                          </div>
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            <span className={`w-2.5 h-2.5 rounded-full ${isOnline ? 'bg-emerald-500 animate-pulse' : 'bg-red-500'}`} />
                            <span className={`text-xs font-semibold ${isOnline ? 'text-emerald-500' : 'text-red-500'}`}>
                              {isOnline ? "Conectado" : "Desconectado"}
                            </span>
                          </div>
                        </TableCell>
                        <TableCell className="text-xs text-muted-foreground">
                          {getRelativeTime(peer.last_seen)}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
}
