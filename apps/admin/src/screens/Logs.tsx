import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PageLayout } from "@syntrix/ui";
import { Card, CardContent } from "@syntrix/ui/components/ui/card";
import { Button } from "@syntrix/ui/components/ui/button";
import { RefreshCw } from "lucide-react";

export function Logs() {
  const [logs, setLogs] = useState<string>("Loading...");

  async function load() {
    const text: string = await invoke("get_logs");
    setLogs(text);
  }
  useEffect(() => { load(); }, []);

  return (
    <PageLayout
      title="Logs del Sistema"
      description="Historial detallado de eventos de sincronización y comunicación P2P."
      actions={
        <Button variant="outline" size="sm" onClick={load} className="flex items-center gap-1.5">
          <RefreshCw size={14} />
          Actualizar
        </Button>
      }
    >
      <Card>
        <CardContent className="p-0">
          <pre className="text-xs font-mono p-4 max-h-[70vh] overflow-y-auto whitespace-pre-wrap break-all bg-muted/20">
            {logs}
          </pre>
        </CardContent>
      </Card>
    </PageLayout>
  );
}
