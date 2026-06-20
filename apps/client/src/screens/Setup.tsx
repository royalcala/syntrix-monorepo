import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Input } from "../components/ui/input";
import { Button } from "../components/ui/button";
import { QrCode, Link, RefreshCw } from "lucide-react";

export function Setup({ nodeId, onJoined }: { nodeId: string; onJoined: () => void }) {
  const [ticket, setTicket] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  async function join() {
    if (!ticket.trim()) return;
    setLoading(true); setError("");
    try {
      await invoke("join_org", { ticketStr: ticket.trim() });
      onJoined();
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-muted to-primary/5 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-primary/10 mb-4">
            <QrCode size={32} className="text-primary" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight">Syntrix</h1>
          <p className="text-muted-foreground mt-2">ERP Client</p>
        </div>

        <div className="bg-card rounded-xl border border-border shadow-sm p-6 space-y-6">
          <div>
            <h2 className="text-lg font-semibold mb-1">Your Device ID</h2>
            <p className="text-sm text-muted-foreground mb-3">Share this with your admin to get invited to an org.</p>
            <code className="block bg-muted rounded-lg p-3 text-sm font-mono break-all">{nodeId}</code>
          </div>

          <div className="border-t border-border pt-4">
            <h3 className="text-sm font-semibold mb-3">Already invited? Paste your ticket</h3>
            <div className="space-y-3">
              <Input
                placeholder="Paste ticket from admin..."
                value={ticket}
                onChange={(e) => setTicket(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && join()}
              />
              <Button className="w-full" onClick={join} disabled={loading || !ticket.trim()}>
                <Link size={16} />
                {loading ? "Joining..." : "Join Organization"}
              </Button>
              {error && <p className="text-sm text-destructive text-center">{error}</p>}
            </div>
          </div>
        </div>

        <button
          onClick={onJoined}
          className="mt-6 mx-auto flex items-center gap-2 text-sm text-muted-foreground hover:text-muted-foreground transition-colors"
        >
          <RefreshCw size={14} /> Check status
        </button>
      </div>
    </div>
  );
}
