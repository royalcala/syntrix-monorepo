import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Card, CardContent } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { RefreshCw } from "lucide-react";

export function Logs() {
  const [logs, setLogs] = useState<string>("Loading...");

  async function load() {
    const text: string = await invoke("get_logs");
    setLogs(text);
  }
  useEffect(() => { load(); }, []);

  return (
    <Card>
      <div className="px-6 py-5 border-b flex items-center justify-between">
        <h3 className="text-lg font-semibold">Logs</h3>
        <Button variant="ghost" size="icon" onClick={load}><RefreshCw size={16} /></Button>
      </div>
      <CardContent className="p-0">
        <pre className="text-xs font-mono p-4 max-h-[70vh] overflow-y-auto whitespace-pre-wrap break-all">
          {logs}
        </pre>
      </CardContent>
    </Card>
  );
}
