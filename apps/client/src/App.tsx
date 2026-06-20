import { useState, useEffect } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FileText, Package, Users, ShoppingCart, Mail, Building2, Copy, Check } from "lucide-react";
import { AppShell, type NavItem, type OrgInfo } from "../../packages/syntrix-ui/src/components/AppShell";
import { Button } from "../../packages/syntrix-ui/src/components/ui/button";
import { Badge } from "../../packages/syntrix-ui/src/components/ui/badge";
import { Inbox } from "./screens/Inbox";
import { EntityGrid } from "./components/EntityGrid";
import { customersEntity } from "./entities/customers";
import { invoicesEntity } from "./entities/invoices";
import { productsEntity } from "./entities/products";
import { ordersEntity } from "./entities/orders";

type InvitePayload = { org_name: string; role: string; tickets: { ns: string; ticket: string }[] };

const entityMap: Record<string, typeof customersEntity> = {
  customers: customersEntity, invoices: invoicesEntity,
  products: productsEntity, orders: ordersEntity,
} as Record<string, typeof customersEntity>;

const navItems: NavItem[] = [
  { href: "/customers", label: "Clientes", icon: Users, section: "org" },
  { href: "/invoices", label: "Facturas", icon: FileText, section: "org" },
  { href: "/products", label: "Productos", icon: Package, section: "org" },
  { href: "/orders", label: "Órdenes", icon: ShoppingCart, section: "org" },
];

const deviceNavItems: NavItem[] = [
  { href: "/inbox", label: "Inbox", icon: Mail, section: "device" },
  { href: "/orgs", label: "Mis Orgs", icon: Building2, section: "device" },
];

export default function App() {
  const navigate = useNavigate();
  const location = useLocation();
  const [nodeId, setNodeId] = useState<string>("");
  const [orgs, setOrgs] = useState<OrgInfo[]>([]);
  const [activeOrg, setActiveOrg] = useState<string>("");
  const [invites, setInvites] = useState<InvitePayload[]>([]);

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

  const isEntityRoute = navItems.some((i) => location.pathname === i.href);

  return (
    <AppShell
      appName="Syntrix"
      appSubtitle="Client"
      nodeId={nodeId}
      orgs={orgs}
      activeOrg={activeOrg}
      onSelectOrg={(id) => { setActiveOrg(id); invoke("set_active_org", { orgId: id }); }}
      onRefresh={loadOrgs}
      navItems={deviceItems}
      extraNavItems={navItems}
    >
      <main className={isEntityRoute ? "h-[calc(100vh-4rem)]" : "p-4 md:p-6"}>
        <Routes>
          <Route path="/inbox" element={<Inbox invites={invites} onAccept={async (invite) => {
            await invoke("join_org", { inviteJson: JSON.stringify(invite), orgName: invite.org_name });
            setInvites((prev) => prev.filter((i) => i !== invite));
            loadOrgs();
          }} />} />
          <Route path="/orgs" element={<OrgsScreen nodeId={nodeId} orgs={orgs} setActiveOrg={(id) => { setActiveOrg(id); invoke("set_active_org", { orgId: id }); }} />} />
          <Route index element={<EntityGrid entity={customersEntity} orgId={activeOrg} role={role} />} />
          <Route path="/customers" element={<EntityGrid entity={customersEntity} orgId={activeOrg} role={role} />} />
          <Route path="/invoices" element={<EntityGrid entity={invoicesEntity} orgId={activeOrg} role={role} />} />
          <Route path="/products" element={<EntityGrid entity={productsEntity} orgId={activeOrg} role={role} />} />
          <Route path="/orders" element={<EntityGrid entity={ordersEntity} orgId={activeOrg} role={role} />} />
        </Routes>
      </main>
    </AppShell>
  );
}

function OrgsScreen({ nodeId, orgs, setActiveOrg }: { nodeId: string; orgs: OrgInfo[]; setActiveOrg: (id: string) => void }) {
  const [addr, setAddr] = useState<string>("");
  const [copied, setCopied] = useState(false);
  useEffect(() => { invoke<string>("get_endpoint_addr").then(setAddr); }, []);
  return (
    <div className="space-y-6 p-4 md:p-6">
      <h2 className="text-xl font-semibold">Mis Organizaciones</h2>
      <div className="bg-card rounded-xl border border-border p-4">
        <div className="flex items-center gap-2 mb-2">
          <p className="text-xs text-muted-foreground">Your Device Address</p>
          <Button variant="ghost" size="sm" className="h-6 px-2 text-xs" onClick={async () => {
            await navigator.clipboard.writeText(addr); setCopied(true); setTimeout(() => setCopied(false), 2000);
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
                 onClick={() => { setActiveOrg(o.id); window.location.hash = "/"; }}>
              <h3 className="font-semibold text-lg">{o.name}</h3>
              <Badge variant="outline" className="mt-2">{o.role}</Badge>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
