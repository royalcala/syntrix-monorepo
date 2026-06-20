import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Input } from "../components/ui/input";
import { Button } from "../components/ui/button";
import { Shield, Rocket } from "lucide-react";

export function CreateOrg({ nodeId, onCreated }: { nodeId: string; onCreated: () => void }) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  async function handleCreate() {
    if (!name.trim()) return;
    setLoading(true);
    setError("");
    try {
      await invoke("create_org", { name: name.trim() });
      onCreated();
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
            <Shield size={32} className="text-primary" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight">Syntrix</h1>
          <p className="text-muted-foreground mt-2">Admin Console</p>
        </div>

        <div className="bg-card rounded-xl border border-border shadow-sm p-6">
          <h2 className="text-lg font-semibold mb-1">Create your first organization</h2>
          <p className="text-sm text-muted-foreground mb-6">An org groups your devices, roles, and data namespaces.</p>

          <div className="space-y-4">
            <div>
              <label className="text-sm font-medium mb-1.5 block">Organization name</label>
              <Input
                placeholder="e.g. Acme Corp"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleCreate()}
                autoFocus
              />
            </div>

            <Button className="w-full" onClick={handleCreate} disabled={loading || !name.trim()}>
              <Rocket size={16} />
              {loading ? "Creating..." : "Create Organization"}
            </Button>

            {error && <p className="text-sm text-destructive text-center">{error}</p>}
          </div>
        </div>

        <p className="text-center text-xs text-muted-foreground mt-6">
          Device: <code className="bg-secondary px-1.5 py-0.5 rounded text-xs font-mono">{nodeId.slice(0, 16)}...</code>
        </p>
      </div>
    </div>
  );
}
