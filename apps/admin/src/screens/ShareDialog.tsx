import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "../components/ui/button";
import { Share2, Copy, Check } from "lucide-react";

export function ShareDialog({ org }: { org: string }) {
  const [tickets, setTickets] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  async function share() {
    const t: string[] = await invoke("share_org", { org });
    setTickets(t);
    setOpen(true);
  }

  async function copyAll() {
    // Join all tickets with newlines — client's join_org parses them
    await navigator.clipboard.writeText(tickets.join("\n"));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  const combined = tickets.join("\n");

  return (
    <>
      <Button variant="outline" size="sm" onClick={share}>
        <Share2 size={16} /> Share
      </Button>

      {open && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={() => setOpen(false)}>
          <div className="bg-card rounded-xl shadow-lg max-w-lg w-full p-6" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold mb-1">Invite to {org}</h3>
            <p className="text-sm text-muted-foreground mb-4">
              Copy this and send it to the user. They paste it in Syntrix Client to join.
            </p>

            <div className="bg-muted rounded-lg p-4 mb-4 max-h-48 overflow-y-auto">
              <code className="text-xs font-mono break-all whitespace-pre-wrap">{combined}</code>
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>Close</Button>
              <Button size="sm" onClick={copyAll}>
                {copied ? <Check size={16} /> : <Copy size={16} />}
                {copied ? "Copied" : "Copy"}
              </Button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
