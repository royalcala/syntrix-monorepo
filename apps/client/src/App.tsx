import { useState, useEffect } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LayoutDashboard, FileText, Package, Users, ShoppingCart, Menu, X, RefreshCw, Mail, Building2, Copy, Check } from "lucide-react";
import { Button } from "./components/ui/button";
import { Badge } from "./components/ui/badge";
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from "./components/ui/select";
import { Sheet } from "./components/ui/sheet";
import { Inbox } from "./screens/Inbox";
import { EntityGrid } from "./components/EntityGrid";
import { ThemeToggle } from "./components/ThemeToggle";
import { customersEntity } from "./entities/customers";
import { invoicesEntity } from "./entities/invoices";
import { productsEntity } from "./entities/products";
import { ordersEntity } from "./entities/orders";

type OrgInfo = { id: string; name: string; role: string };
type InvitePayload = { org_name: string; role: string; tickets: { ns: string; ticket: string }[] };

const entityMap: Record<string, typeof customersEntity> = {
  customers: customersEntity,
  invoices: invoicesEntity,
  products: productsEntity,
  orders: ordersEntity,
} as Record<string, typeof customersEntity>;

const navItems = [
  { href: "/customers", label: "Clientes", icon: Users, id: "customers" },
  { href: "/invoices", label: "Facturas", icon: FileText, id: "invoices" },
  { href: "/products", label: "Productos", icon: Package, id: "products" },
  { href: "/orders", label: "Órdenes", icon: ShoppingCart, id: "orders" },
];

export default function App() {
  const [nodeId, setNodeId] = useState<string>("");
  const [orgs, setOrgs] = useState<OrgInfo[]>([]);
  const [activeOrg, setActiveOrg] = useState<string>("");
  const [invites, setInvites] = useState<InvitePayload[]>([]);

  useEffect(() => {
    invoke<string>("get_node_id").then(setNodeId);
    loadOrgs();
    const unlisten = listen<InvitePayload>("invite-received", (event) => {
      console.log("invite-received event:", event.payload);
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

  return (
    <Layout
      nodeId={nodeId} orgs={orgs} activeOrg={activeOrg}
      setActiveOrg={(id) => { setActiveOrg(id); invoke("set_active_org", { orgId: id }); }}
      onRefresh={loadOrgs} invites={invites}
      onAcceptInvite={async (invite) => {
        await invoke("join_org", { inviteJson: JSON.stringify(invite), orgName: invite.org_name });
        setInvites((prev) => prev.filter((i) => i !== invite));
        loadOrgs();
      }}
    />
  );
}

function Layout({
  nodeId, orgs, activeOrg, setActiveOrg, onRefresh, invites, onAcceptInvite,
}: {
  nodeId: string; orgs: OrgInfo[]; activeOrg: string; setActiveOrg: (id: string) => void;
  onRefresh: () => void; invites: InvitePayload[];
  onAcceptInvite: (invite: InvitePayload) => void;
}) {
  const navigate = useNavigate();
  const location = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const role = orgs.find((o) => o.id === activeOrg)?.role;
  const activeOrgName = orgs.find((o) => o.id === activeOrg)?.name ?? activeOrg;

  const currentPath = location.pathname;
  const activeItem = navItems.find((i) => currentPath === i.href || currentPath.startsWith(i.href + "/"));
  const currentLabel = activeItem?.label
    ?? (currentPath === "/inbox" ? "Inbox" : "Mis Orgs");

  const entityPage = activeItem && entityMap[activeItem.id];
  const isEntityRoute = !!entityPage;

  const sidebarContent = (
    <>
      <div className="px-6 py-5 border-b border-border flex items-center justify-between">
        <div>
          <h1 className="text-lg font-bold tracking-tight">Syntrix</h1>
          <p className="text-xs text-muted-foreground mt-0.5">Client</p>
        </div>
        <button onClick={() => setSidebarOpen(false)} className="lg:hidden p-1 rounded-md hover:bg-accent">
          <X size={18} />
        </button>
      </div>

      <div className="px-3 py-4 flex flex-col flex-1 overflow-y-auto">
        <div className="mb-4">
          <p className="text-xs text-muted-foreground uppercase tracking-wider px-3 mb-2">Device</p>
          <nav className="flex flex-col gap-1">
            <button onClick={() => { navigate("/inbox"); setSidebarOpen(false); }}
              className={`flex items-center gap-3 px-3 py-2.5 text-sm rounded-lg transition-colors ${
                currentPath === "/inbox" ? "bg-primary/10 text-primary font-medium" : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
              }`}>
              <Mail size={18} /> Inbox
              {invites.length > 0 && <Badge variant="destructive" className="ml-auto">{invites.length}</Badge>}
            </button>
            <button onClick={() => { navigate("/orgs"); setSidebarOpen(false); }}
              className={`flex items-center gap-3 px-3 py-2.5 text-sm rounded-lg transition-colors ${
                currentPath === "/orgs" || currentPath === "/" ? "bg-primary/10 text-primary font-medium" : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
              }`}>
              <Building2 size={18} /> Mis Orgs
            </button>
          </nav>
        </div>

        {orgs.length > 0 && (
          <div className="px-3 mb-4 pt-2 border-t border-border">
            <Select value={activeOrg} onValueChange={(v) => { setActiveOrg(v); setSidebarOpen(false); }}>
              <SelectTrigger><SelectValue placeholder="Select org" /></SelectTrigger>
              <SelectContent>
                {orgs.map((o) => (<SelectItem key={o.id} value={o.id}>{o.name}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
        )}

        {activeOrg && (
          <div>
            <p className="text-xs text-muted-foreground uppercase tracking-wider px-3 mb-2">{activeOrgName}</p>
            <nav className="flex flex-col gap-1">
              {navItems.map((item) => (
                <button key={item.href}
                  onClick={() => { navigate(item.href); setSidebarOpen(false); }}
                  className={`flex items-center gap-3 px-3 py-2.5 text-sm rounded-lg transition-colors ${
                    currentPath === item.href || currentPath.startsWith(item.href + "/")
                      ? "bg-primary/10 text-primary font-medium" : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                  }`}>
                  <item.icon size={18} /> {item.label}
                </button>
              ))}
            </nav>
          </div>
        )}
      </div>

      <div className="mt-auto px-3 py-4 border-t border-border">
        <div className="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground">
          <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
          {nodeId.slice(0, 16)}...
        </div>
        <div className="flex items-center px-3 py-1">
          <ThemeToggle />
          <span className="text-xs text-muted-foreground ml-2">Toggle theme</span>
        </div>
      </div>
    </>
  );

  return (
    <div className="min-h-screen bg-background">
      <Sheet open={sidebarOpen} onClose={() => setSidebarOpen(false)}>
        {sidebarContent}
      </Sheet>

      <aside className="hidden lg:flex w-64 bg-sidebar text-sidebar-foreground border-r border-border flex-col fixed inset-y-0 left-0 z-30">
        {sidebarContent}
      </aside>

      <div className="lg:pl-64">
        <header className="h-16 border-b border-border bg-background flex items-center px-4 md:px-6 gap-3 sticky top-0 z-20">
          <button onClick={() => setSidebarOpen(true)} className="lg:hidden p-2 -ml-2 rounded-md hover:bg-accent">
            <Menu size={20} />
          </button>
          <h2 className="text-lg font-semibold truncate">{currentLabel}</h2>
          <div className="ml-auto flex gap-2 items-center">
            <span className="hidden sm:inline text-xs text-muted-foreground mr-2">{activeOrgName} · {role}</span>
            <Button variant="ghost" size="icon" onClick={onRefresh}><RefreshCw size={16} /></Button>
          </div>
        </header>

        <main className={isEntityRoute ? "h-[calc(100vh-4rem)]" : "p-4 md:p-6"}>
          <Routes>
            <Route path="/inbox" element={<Inbox invites={invites} onAccept={onAcceptInvite} />} />
            <Route path="/orgs" element={<OrgsScreen orgs={orgs} navigate={navigate} setActiveOrg={setActiveOrg} />} />
            <Route index element={<OrgsScreen orgs={orgs} navigate={navigate} setActiveOrg={setActiveOrg} />} />
            <Route path="/customers" element={<EntityGrid entity={customersEntity} orgId={activeOrg} role={role} />} />
            <Route path="/invoices" element={<EntityGrid entity={invoicesEntity} orgId={activeOrg} role={role} />} />
            <Route path="/products" element={<EntityGrid entity={productsEntity} orgId={activeOrg} role={role} />} />
            <Route path="/orders" element={<EntityGrid entity={ordersEntity} orgId={activeOrg} role={role} />} />
          </Routes>
        </main>
      </div>
    </div>
  );
}

function OrgsScreen({ orgs, navigate, setActiveOrg }: { orgs: OrgInfo[]; navigate: (path: string) => void; setActiveOrg: (id: string) => void }) {
  const [addr, setAddr] = useState<string>("");
  const [copied, setCopied] = useState(false);

  useEffect(() => { invoke<string>("get_endpoint_addr").then(setAddr); }, []);

  async function copyAddr() {
    await navigator.clipboard.writeText(addr);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold">Mis Organizaciones</h2>
      <div className="bg-card rounded-xl border border-border p-4">
        <div className="flex items-center gap-2 mb-2">
          <p className="text-xs text-muted-foreground">Your Device Address (share with admin)</p>
          <Button variant="ghost" size="sm" className="h-6 px-2 text-xs" onClick={copyAddr}>
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? "Copied" : "Copy"}
          </Button>
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
  );
}
