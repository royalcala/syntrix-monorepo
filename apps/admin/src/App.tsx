import { useState, useEffect } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Users, Shield, Building2, Terminal, Plus } from "lucide-react";
import { AppShell, type NavItem } from "@syntrix/ui/components/AppShell";
import { Button } from "@syntrix/ui/components/ui/button";
import { CreateOrg } from "./screens/CreateOrg";
import { Logs } from "./screens/Logs";
import { ShareDialog } from "./screens/ShareDialog";
import { DevicesGridPage } from "./screens/DevicesGridPage";
import { RolesGridPage } from "./screens/RolesGridPage";
import { OrgsGridPage } from "./screens/OrgsGridPage";

type OrgInfo = { name: string };

const navItems: NavItem[] = [
  { href: "/devices", label: "Dispositivos", icon: Users },
  { href: "/roles", label: "Roles", icon: Shield },
  { href: "/orgs", label: "Organizaciones", icon: Building2 },
  { href: "/logs", label: "Logs", icon: Terminal },
];

export default function App() {
  const [nodeId, setNodeId] = useState<string>("");
  const [orgs, setOrgs] = useState<OrgInfo[]>([]);
  const [activeOrg, setActiveOrg] = useState<string>("");
  const [newOrgOpen, setNewOrgOpen] = useState(false);
  const [newOrgName, setNewOrgName] = useState("");

  useEffect(() => {
    invoke<string>("get_node_id").then(setNodeId);
    loadOrgs();
  }, []);

  async function loadOrgs() {
    try {
      const list: OrgInfo[] = await invoke("list_orgs");
      setOrgs(list);
      if (list.length > 0 && !activeOrg) setActiveOrg(list[0].name);
    } catch {}
  }

  if (orgs.length === 0) {
    return <CreateOrg nodeId={nodeId} onCreated={loadOrgs} />;
  }

  return (
    <>
      <AppShell
        appName="Syntrix"
        appSubtitle="Admin Console"
        nodeId={nodeId}
        orgs={orgs.map((o) => ({ id: o.name, name: o.name }))}
        activeOrg={activeOrg}
        onSelectOrg={setActiveOrg}
        navItems={navItems}
        headerActions={
          <>
            <ShareDialog org={activeOrg} />
            <Button size="sm" variant="outline" onClick={() => setNewOrgOpen(true)}>
              <Plus size={14} /> <span className="hidden sm:inline ml-1">New Org</span>
            </Button>
          </>
        }>
        <main className="h-[calc(100vh-3.5rem)]">
          <Routes>
            <Route index element={<DevicesGridPage org={activeOrg} />} />
            <Route path="/devices" element={<DevicesGridPage org={activeOrg} />} />
            <Route path="/roles" element={<RolesGridPage org={activeOrg} />} />
            <Route path="/orgs" element={<OrgsGridPage />} />
            <Route path="/logs" element={<Logs />} />
          </Routes>
        </main>
      </AppShell>

      {newOrgOpen && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setNewOrgOpen(false)}>
          <div className="bg-card rounded-xl shadow-lg max-w-sm w-full mx-4 p-6" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold mb-4">Create Organization</h3>
            <input
              className="w-full h-10 rounded-md border px-3 py-2 text-sm mb-4 bg-background"
              placeholder="Organization name"
              value={newOrgName}
              onChange={(e) => setNewOrgName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && createNewOrg()}
              autoFocus />
            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setNewOrgOpen(false)}>Cancel</Button>
              <Button size="sm" onClick={createNewOrg}>Create</Button>
            </div>
          </div>
        </div>
      )}
    </>
  );

  async function createNewOrg() {
    if (!newOrgName.trim()) return;
    await invoke("create_org", { name: newOrgName.trim() });
    setActiveOrg(newOrgName.trim());
    setNewOrgName("");
    setNewOrgOpen(false);
    await loadOrgs();
  }
}
