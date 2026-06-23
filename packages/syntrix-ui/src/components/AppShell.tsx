import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Menu, X, RefreshCw } from "lucide-react";
import { Button } from "./ui/button";
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from "./ui/select";
import { Badge } from "./ui/badge";
import { Sheet } from "./ui/sheet";
import { ThemeToggle } from "./ThemeToggle";
import { cn } from "../lib/utils";

export interface NavItem {
  href: string;
  label: string;
  icon: React.ComponentType<{ size?: number }>;
  badge?: number;
  section?: "device" | "org";
}

export interface OrgInfo {
  id: string;
  name: string;
  role?: string;
}

interface AppShellProps {
  appName: string;
  appSubtitle: string;
  nodeId: string;
  orgs: OrgInfo[];
  activeOrg: string;
  onSelectOrg: (id: string) => void;
  onRefresh?: () => void;
  navItems: NavItem[];
  extraNavItems?: NavItem[];
  headerActions?: React.ReactNode;
  children: React.ReactNode;
  onOpenCommand?: () => void;
  syncIndicator?: React.ReactNode;
}

export function AppShell({
  appName, appSubtitle, nodeId, orgs, activeOrg, onSelectOrg,
  onRefresh, onOpenCommand, navItems, extraNavItems, headerActions, children,
  syncIndicator,
}: AppShellProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const activeOrgName = orgs.find((o) => o.id === activeOrg)?.name ?? activeOrg;
  const role = orgs.find((o) => o.id === activeOrg)?.role;

  const isActive = (item: NavItem) =>
    location.pathname === item.href || location.pathname.startsWith(item.href + "/");

  const activeItem = [...navItems, ...(extraNavItems ?? [])].find(isActive);
  const currentLabel = activeItem?.label
    ?? (location.pathname === "/inbox" ? "Inbox" : location.pathname === "/orgs" ? "Mis Orgs" : appName);

  // Ctrl+K → Command Palette
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") { e.preventDefault(); if (onOpenCommand) onOpenCommand(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onOpenCommand]);

  const renderNav = (items: NavItem[]) =>
    items.map((item) => (
      <button
        key={item.href}
        onClick={() => { navigate(item.href); setSidebarOpen(false); }}
        className={cn(
          "flex items-center gap-3 px-3 py-2.5 text-sm rounded-lg transition-colors w-full text-left",
          isActive(item)
            ? "bg-primary/10 text-primary font-medium"
            : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        )}>
        <item.icon size={18} />
        {item.label}
        {item.badge ? <Badge variant="destructive" className="ml-auto">{item.badge}</Badge> : null}
      </button>
    ));

  const deviceItems = navItems.filter((i) => i.section === "device");
  const orgItems = navItems.filter((i) => i.section !== "device");

  const sidebarContent = (
    <>
      <div className="px-6 py-5 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-2">
          <img src="/favicon.svg" alt="" className="w-5 h-5" />
          <div>
            <h1 className="text-lg font-bold tracking-tight">{appName}</h1>
            <p className="text-xs text-muted-foreground mt-0.5">{appSubtitle}</p>
          </div>
        </div>
        <button onClick={() => setSidebarOpen(false)} className="lg:hidden p-1 rounded-md hover:bg-accent">
          <X size={18} />
        </button>
      </div>

      <div className="px-3 py-4 flex flex-col flex-1 overflow-y-auto">
        {deviceItems.length > 0 && (
          <div className="mb-4">
            <p className="text-xs text-muted-foreground uppercase tracking-wider px-3 mb-2">Device</p>
            <nav className="flex flex-col gap-1">{renderNav(deviceItems)}</nav>
          </div>
        )}

        {orgs.length > 0 && (
          <div className="px-3 mb-4 pt-2 border-t border-border">
            <Select value={activeOrg} onValueChange={(v) => { onSelectOrg(v); setSidebarOpen(false); }}>
              <SelectTrigger><SelectValue placeholder="Select org" /></SelectTrigger>
              <SelectContent>
                {orgs.map((o) => (<SelectItem key={o.id} value={o.id}>{o.name}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
        )}

        {activeOrg && orgItems.length > 0 && (
          <div>
            <p className="text-xs text-muted-foreground uppercase tracking-wider px-3 mb-2">{activeOrgName}</p>
            <nav className="flex flex-col gap-1">{renderNav(orgItems)}</nav>
          </div>
        )}

        {extraNavItems && extraNavItems.length > 0 && (
          <div className="mt-4 pt-2 border-t border-border">
            <nav className="flex flex-col gap-1">{renderNav(extraNavItems)}</nav>
          </div>
        )}
      </div>

      <div className="mt-auto px-3 py-4 border-t border-border space-y-3">
        {syncIndicator && <div className="px-1">{syncIndicator}</div>}
        <div className="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground bg-accent/50 rounded-md">
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

      <aside className="max-lg:hidden w-64 bg-card border-r border-border flex-col fixed inset-y-0 left-0 z-30 flex">
        {sidebarContent}
      </aside>

      <div className="lg:pl-64">
        <header className="h-16 border-b border-border bg-background flex items-center px-4 md:px-6 gap-3 sticky top-0 z-20">
          <button onClick={() => setSidebarOpen(true)} className="lg:hidden p-2 -ml-2 rounded-md hover:bg-accent">
            <Menu size={20} />
          </button>
          <h2 className="text-lg font-semibold truncate">{currentLabel}</h2>
          <div className="ml-auto flex gap-2 items-center">
            {role && <span className="hidden sm:inline text-xs text-muted-foreground mr-2">{activeOrgName} · {role}</span>}
            {onRefresh && (
              <Button variant="ghost" size="icon" onClick={onRefresh}><RefreshCw size={16} /></Button>
            )}
            {headerActions}
          </div>
        </header>

        <main>{children}</main>
      </div>
    </div>
  );
}