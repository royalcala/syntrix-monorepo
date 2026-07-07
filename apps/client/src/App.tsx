import { useState, useEffect, useMemo } from "react";
import { Routes, Route, useNavigate, Navigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useQuery } from "@tanstack/react-query";
import { FileText, Package, Users, ShoppingCart, Mail, Building2, Copy, Check, Home } from "lucide-react";
import { AppShell, type NavItem, type OrgInfo } from "@syntrix/ui/components/AppShell";
import { SyncStatusIndicator } from "@syntrix/ui/components/SyncStatusIndicator";
import { SyncDetailsPage } from "@syntrix/ui/components/SyncDetailsPage";
import { PageLayout } from "@syntrix/ui/components/PageLayout";
import { CommandPalette } from "@syntrix/ui/components/CommandPalette";
import { Button } from "@syntrix/ui/components/ui/button";
import { Badge } from "@syntrix/ui/components/ui/badge";
import { HomeScreen } from "./screens/HomeScreen";
import { ViewScreen } from "./screens/ViewScreen";
import { Inbox } from "./screens/Inbox";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { useTimelineCursor } from "./hooks/useTimelineCursor";
import { useIAQueue } from "./hooks/useIAQueue";
import { customersEntity } from "./entities/customers";
import { invoicesEntity } from "./entities/invoices";
import { productsEntity } from "./entities/products";
import { ordersEntity } from "./entities/orders";

type InvitePayload = { org_name: string; role: string; admin_addr?: string; tickets: { ns: string; ticket: string }[] };


const navItems: NavItem[] = [
  { href: "/customers", label: "Clientes", icon: Users, section: "org" },
  { href: "/invoices", label: "Facturas", icon: FileText, section: "org" },
  { href: "/products", label: "Productos", icon: Package, section: "org" },
  { href: "/orders", label: "Órdenes", icon: ShoppingCart, section: "org" },
];

const deviceNavItems: NavItem[] = [
  { href: "/", label: "Inicio", icon: Home, section: "device" },
  { href: "/inbox", label: "Inbox", icon: Mail, section: "device" },
  { href: "/orgs", label: "Mis Orgs", icon: Building2, section: "device" },
];

export default function App() {
  const navigate = useNavigate();
  const [nodeId, setNodeId] = useState<string>("");
  const [orgs, setOrgs] = useState<OrgInfo[]>([]);
  const [activeOrg, setActiveOrg] = useState<string>("");
  const [invites, setInvites] = useState<InvitePayload[]>([]);
  const [cmdOpen, setCmdOpen] = useState(false);
  const [cmdQuery, setCmdQuery] = useState("");
  const [debouncedCmdQuery, setDebouncedCmdQuery] = useState("");

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedCmdQuery(cmdQuery);
    }, 150);
    return () => clearTimeout(handler);
  }, [cmdQuery]);

  const { data: dbSearchResults, isLoading: isSearchLoading } = useQuery({
    queryKey: ["global-search", activeOrg, debouncedCmdQuery],
    queryFn: async () => {
      if (!debouncedCmdQuery.trim()) return [];
      try {
        const results = await invoke<any[]>("search_entity", {
          orgId: activeOrg,
          query: debouncedCmdQuery,
          limit: 10,
        });
        return results;
      } catch (err) {
        console.error("Global search failed:", err);
        return [];
      }
    },
    enabled: !!activeOrg && !!debouncedCmdQuery.trim(),
  });

  const searchResults = useMemo(() => {
    const navMatches = cmdQuery
      ? navItems
          .filter((i) => i.label.toLowerCase().includes(cmdQuery.toLowerCase()))
          .map((i) => ({
            id: i.href,
            label: i.label,
            entity: "nav",
            entityLabel: "Navegar",
          }))
      : [];

    const dbMatches = (dbSearchResults || []).map((r: any) => ({
      id: r.doc_id,
      label: r.title,
      subtitle: r.snippet,
      entity: r.entity,
      entityLabel:
        r.entity === "customers"
          ? "Cliente"
          : r.entity === "invoices"
          ? "Factura"
          : r.entity === "products"
          ? "Producto"
          : r.entity === "orders"
          ? "Orden"
          : r.entity,
    }));

    return [...navMatches, ...dbMatches];
  }, [cmdQuery, dbSearchResults]);

  const handleSelectResult = (r: any) => {
    if (r.entity === "nav") {
      navigate(r.id);
    } else {
      navigate(`/${r.entity}?id=${r.id}`);
    }
  };

  useEffect(() => {
    invoke<string>("get_node_id").then(setNodeId);
    loadOrgs();
    const unlisten = listen<InvitePayload>("invite-received", (event) => {
      setInvites((prev) => [...prev, event.payload]);
    });
    return () => { unlisten.then((u) => u()); };
  }, []);

  async function loadOrgs() {
    try {
      const list: OrgInfo[] = await invoke("list_orgs");
      setOrgs(list);
      if (list.length > 0 && !activeOrg) {
        setActiveOrg(list[0].id);
        invoke("set_active_org", { orgId: list[0].id });
      }
    } catch {}
  }

  const role = orgs.find((o) => o.id === activeOrg)?.role;
  const inboxBadge = invites.length > 0 ? invites.length : undefined;
  const deviceItems: NavItem[] = [
    { ...deviceNavItems[0]!, badge: inboxBadge },
    ...deviceNavItems.slice(1),
  ];

  const { data: roleData } = useQuery({
    queryKey: ["entity", "roles", activeOrg, role],
    queryFn: async () => {
      try {
        const list: any[] = await invoke("query_entity", { orgId: activeOrg, entity: "roles" });
        console.log("Roles found in control doc:", list);
        return list.find((r) => r.name === role) || { can_open: [], can_write: [] };
      } catch (e) {
        console.error("Error fetching roles:", e);
        return { can_open: [], can_write: [] };
      }
    },
    enabled: !!activeOrg && !!role,
    // Re-query every 15 seconds to catch role changes that arrived via sync
    // (sync may not complete before the initial render, especially for 2nd+ peers)
    refetchInterval: 15_000,
  });

  // Timeline cursor: poll CDC changes for the active org and invalidate
  // react-query caches when remote peers make changes
  useTimelineCursor(activeOrg);

  // IA queue: poll completed ia_queries and show notifications
  useIAQueue(activeOrg);

  const filteredNavItems = navItems.filter((item) => {
    if (role === "admin") return true; // El admin local siempre ve todo por seguridad
    if (!roleData) return false;
    const canOpen = (roleData.can_open as string[]) || [];
    const moduleName = item.href.substring(1); // ej. "/customers" -> "customers"
    return canOpen.includes(moduleName);
  });

  return (
    <AppShell
      appName="Syntrix"
      appSubtitle="Client"
      nodeId={nodeId}
      orgs={orgs}
      activeOrg={activeOrg}
      onSelectOrg={(id: string) => { setActiveOrg(id); invoke("set_active_org", { orgId: id }); }}
      onRefresh={loadOrgs}
      onOpenCommand={() => setCmdOpen(true)}
      navItems={deviceItems}
      extraNavItems={filteredNavItems}
      syncIndicator={activeOrg ? <SyncStatusIndicator org={activeOrg} /> : null}
    >
      <main className="h-[calc(100vh-4rem)] lg:h-screen flex flex-col overflow-y-auto">
        <Routes>
          <Route path="/" element={<HomeScreen org={activeOrg} />} />
          <Route path="/view/:viewId" element={<ViewScreen org={activeOrg} />} />
          <Route path="/inbox" element={<Inbox invites={invites} onAccept={async (invite) => {
            await invoke("join_org", { inviteJson: JSON.stringify(invite), orgName: invite.org_name });
            setInvites((prev) => prev.filter((i) => i !== invite));
            loadOrgs();
          }} />} />
          <Route path="/orgs" element={<OrgsScreen nodeId={nodeId} orgs={orgs} setActiveOrg={(id) => { setActiveOrg(id); invoke("set_active_org", { orgId: id }); }} />} />
          <Route index element={<Navigate to="/" replace />} />
          <Route path="/customers" element={
            <PageLayout
              title="Clientes"
              description="Administración de la cartera de clientes y cuentas asociadas."
              className="h-full"
              contentClassName="flex-1 min-h-0 flex flex-col"
            >
              <EntityGrid entity={customersEntity} orgId={activeOrg} role={role} enableSearch={true} />
            </PageLayout>
          } />
          <Route path="/invoices" element={
            <PageLayout
              title="Facturas"
              description="Gestión y emisión de facturas para los clientes autorizados."
              className="h-full"
              contentClassName="flex-1 min-h-0 flex flex-col"
            >
              <EntityGrid entity={invoicesEntity} orgId={activeOrg} role={role} enableSearch={true} />
            </PageLayout>
          } />
          <Route path="/products" element={
            <PageLayout
              title="Productos"
              description="Listado y control de inventario de productos y servicios."
              className="h-full"
              contentClassName="flex-1 min-h-0 flex flex-col"
            >
              <EntityGrid entity={productsEntity} orgId={activeOrg} role={role} enableSearch={true} />
            </PageLayout>
          } />
          <Route path="/orders" element={
            <PageLayout
              title="Órdenes"
              description="Administración de pedidos y órdenes de compra de la organización."
              className="h-full"
              contentClassName="flex-1 min-h-0 flex flex-col"
            >
              <EntityGrid entity={ordersEntity} orgId={activeOrg} role={role} enableSearch={true} />
            </PageLayout>
          } />
          <Route path="/sync" element={<SyncDetailsPage org={activeOrg} nodeId={nodeId} />} />
        </Routes>
      </main>
      <CommandPalette
        open={cmdOpen}
        onClose={() => { setCmdOpen(false); setCmdQuery(""); }}
        results={searchResults}
        actions={[
          { id: "new-customer", label: "Nuevo cliente", shortcut: "⌘N", action: () => navigate("/customers") },
          { id: "new-invoice", label: "Nueva factura", action: () => navigate("/invoices") },
          { id: "orgs", label: "Ir a Mis Orgs", action: () => navigate("/orgs") },
          { id: "inbox", label: "Ir a Inbox", action: () => navigate("/inbox") },
        ]}
        onSelectResult={handleSelectResult}
        searchQuery={cmdQuery}
        onSearchChange={setCmdQuery}
        isLoading={isSearchLoading}
      />
    </AppShell>
  );
}

function OrgsScreen({ nodeId: _nodeId, orgs, setActiveOrg }: { nodeId: string; orgs: OrgInfo[]; setActiveOrg: (id: string) => void }) {
  const navigate = useNavigate();
  const [addr, setAddr] = useState<string>("");
  const [copied, setCopied] = useState(false);
  useEffect(() => { invoke<string>("get_endpoint_addr").then(setAddr); }, []);
  return (
    <PageLayout
      title="Mis Organizaciones"
      description="Visualiza tus organizaciones y comparte tu dirección de dispositivo para recibir invitaciones."
    >
      <div className="space-y-6">
        <div className="bg-card rounded-xl border border-border p-4">
          <div className="flex items-center gap-2 mb-2">
            <p className="text-xs text-muted-foreground">Your Device Address</p>
            <Button variant="ghost" size="sm" className="h-6 px-2 text-xs" onClick={async () => {
              try {
                if (navigator.clipboard && window.isSecureContext) {
                  await navigator.clipboard.writeText(addr);
                } else {
                  const textArea = document.createElement("textarea");
                  textArea.value = addr;
                  textArea.style.position = "absolute";
                  textArea.style.left = "-999999px";
                  document.body.prepend(textArea);
                  textArea.select();
                  try { document.execCommand("copy"); } catch (e) { console.error(e); }
                  textArea.remove();
                }
                setCopied(true); setTimeout(() => setCopied(false), 2000);
              } catch (err) {
                console.error(err);
                prompt("El navegador bloqueó el copiado automático. Por favor cópialo de aquí:", addr);
              }
            }}>{copied ? <Check size={12} /> : <Copy size={12} />}{copied ? "Copied" : "Copy"}</Button>
          </div>
          <code className="block text-xs font-mono break-all max-h-16 overflow-y-auto">{addr || "loading..."}</code>
        </div>
        {orgs.length === 0 ? (
          <div className="text-center py-8">
            <Building2 size={40} className="text-muted-foreground mx-auto mb-4" />
            <p className="text-muted-foreground mb-2">You are not a member of any organization yet.</p>
            <p className="text-sm text-muted-foreground">Share your Device Address above with an admin to get invited.</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {orgs.map((o) => (
              <div key={o.id} className="bg-card rounded-xl border border-border p-5 hover:shadow-md transition-shadow cursor-pointer"
                   onClick={() => { setActiveOrg(o.id); navigate("/"); }}>
                <h3 className="font-semibold text-lg">{o.name}</h3>
                <Badge variant="outline" className="mt-2">{o.role}</Badge>
              </div>
            ))}
          </div>
        )}
      </div>
    </PageLayout>
  );
}
