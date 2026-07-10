import { useState, useEffect } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Users, Shield, Building2, Terminal, Database, ScrollText, Plus, TerminalSquare, Eye, Puzzle, UserPlus } from "lucide-react";
import { AppShell, type NavItem } from "@syntrix/ui/components/AppShell";
import { SyncStatusIndicator } from "@syntrix/ui/components/SyncStatusIndicator";
import { SyncDetailsPage } from "@syntrix/ui/components/SyncDetailsPage";
import { PageLayout } from "@syntrix/ui/components/PageLayout";
import { Button } from "@syntrix/ui/components/ui/button";
import { CreateOrg } from "./screens/CreateOrg";
import { Logs } from "./screens/Logs";
import { ShareDialog } from "./screens/ShareDialog";
import { DevicesGridPage } from "./screens/DevicesGridPage";
import { RolesGridPage } from "./screens/RolesGridPage";
import { OrgsGridPage } from "./screens/OrgsGridPage";
import { SchemaExplorer } from "./screens/SchemaExplorer";
import { AuditTrail } from "./screens/AuditTrail";
import { SqlConsole } from "./screens/SqlConsole";
import { ViewsPage } from "./screens/ViewsPage";
import { ModulesPage } from "./screens/ModulesPage";
import { PendingEnrollmentsPage } from "./screens/PendingEnrollmentsPage";

type OrgInfo = { name: string };

const navItems: NavItem[] = [
  { href: "/devices", label: "Dispositivos", icon: Users },
  { href: "/roles", label: "Roles", icon: Shield },
  { href: "/orgs", label: "Organizaciones", icon: Building2 },
  { href: "/enrollments", label: "Solicitudes", icon: UserPlus },
  { href: "/views", label: "Vistas IA", icon: Eye },
  { href: "/modules", label: "Módulos", icon: Puzzle },
  { href: "/schemas", label: "Esquemas", icon: Database },
  { href: "/audit", label: "Auditoría", icon: ScrollText },
  { href: "/sql", label: "Consola SQL", icon: TerminalSquare },
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
        syncIndicator={activeOrg ? <SyncStatusIndicator org={activeOrg} /> : null}>
          <Routes>
            <Route index element={<Navigate to="/devices" replace />} />
            <Route path="/devices" element={
              <PageLayout
                title="Dispositivos"
                description="Monitorea y autoriza dispositivos conectados en la red P2P de esta organización."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
                actions={<ShareDialog org={activeOrg} />}
              >
                <DevicesGridPage org={activeOrg} />
              </PageLayout>
            } />
            <Route path="/roles" element={
              <PageLayout
                title="Roles"
                description="Define y administra los roles y niveles de acceso a la base de datos."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
              >
                <RolesGridPage org={activeOrg} />
              </PageLayout>
            } />
            <Route path="/orgs" element={
              <PageLayout
                title="Organizaciones"
                description="Administra todas las organizaciones locales registradas en este nodo."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
                actions={
                  <Button size="sm" variant="outline" onClick={() => setNewOrgOpen(true)}>
                    <Plus size={14} /> <span className="ml-1">New Org</span>
                  </Button>
                }
              >
                <OrgsGridPage />
              </PageLayout>
            } />
            <Route path="/schemas" element={
              <PageLayout
                title="Explorador de Esquemas"
                description="Visualiza la definición de entidades, campos, índices y relaciones del registry de esquemas."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0"
              >
                <SchemaExplorer />
              </PageLayout>
            } />
            <Route path="/views" element={
              <PageLayout
                title="Vistas IA"
                description="Vistas generadas por la IA. Explora, ejecuta y administra las vistas guardadas."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
              >
                <ViewsPage org={activeOrg} />
              </PageLayout>
            } />
            <Route path="/enrollments" element={
              <PageLayout
                title="Solicitudes de Enrolamiento"
                description="Aproba o rechaza solicitudes de dispositivos que se conectan vía QR. Las solicitudes aparecen cuando un cliente escanea el código de la organización."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
              >
                <PendingEnrollmentsPage />
              </PageLayout>
            } />
            <Route path="/modules" element={
              <PageLayout
                title="Módulos"
                description="Templates y módulos activos. Los módulos extienden Syntrix con nuevas entidades y reglas."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0"
              >
                <ModulesPage />
              </PageLayout>
            } />
            <Route path="/audit" element={
              <PageLayout
                title="Auditoría de Eventos"
                description="Feed cronológico de todas las mutaciones P2P registradas en el log de eventos."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
              >
                <AuditTrail org={activeOrg} />
              </PageLayout>
            } />
            <Route path="/sql" element={
              <PageLayout
                title="Consola SQL"
                description="Ejecuta consultas SELECT de solo lectura contra la réplica relacional del admin y guarda vistas favoritas."
                className="h-[calc(100vh-4rem)] lg:h-screen"
                contentClassName="flex-1 min-h-0 flex flex-col"
              >
                <SqlConsole />
              </PageLayout>
            } />
            <Route path="/logs" element={<Logs />} />
            <Route path="/sync" element={<SyncDetailsPage org={activeOrg} nodeId={nodeId} />} />
          </Routes>
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
