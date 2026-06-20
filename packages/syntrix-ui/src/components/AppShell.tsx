import { useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Menu, X, RefreshCw, ChevronLeft } from "lucide-react";
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
}

export function AppShell({
  appName, appSubtitle, nodeId, orgs, activeOrg, onSelectOrg,
  onRefresh, navItems, extraNavItems, headerActions, children,
}: AppShellProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);

  const activeOrgName = orgs.find((o) => o.id === activeOrg)?.name ?? activeOrg;
  const role = orgs.find((o) => o.id === activeOrg)?.role;

  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(href + "/");

  const activeItem = [...navItems, ...(extraNavItems ?? [])].find((i) => isActive(i.href));
  const currentLabel = activeItem?.label
    ?? (location.pathname === "/inbox" ? "Inbox" : location.pathname === "/orgs" ? "Mis Orgs" : appName);

  const renderNav = (items: NavItem[]) =>
    items.map((item) => (
      <button
        key={item.href}
        onClick={() => { navigate(item.href); setSidebarOpen(false); }}
        className={cn(
          "flex items-center gap-3 px-3 py-2 text-sm rounded-md transition-colors w-full text-left",
          "hover:bg-accent hover:text-accent-foreground",
          isActive(item.href)
            ? "bg-accent text-accent-foreground font-medium"
            : "text-muted-foreground",
        )}>
        <item.icon size={18} className="shrink-0" />
        {!collapsed && (
          <>
            <span className="truncate">{item.label}</span>
            {item.badge ? <Badge variant="destructive" className="ml-auto">{item.badge}</Badge> : null}
          </>
        )}
        {collapsed && item.badge ? <Badge variant="destructive" className="ml-auto text-[10px] px-1">{item.badge}</Badge> : null}
      </button>
    ));

  const deviceItems = navItems.filter((i) => i.section === "device");
  const orgItems = navItems.filter((i) => i.section !== "device");

  const sidebarContent = (
    <div className={cn("flex flex-col h-full transition-all", collapsed ? "items-center" : "")}>
      {/* Header */}
      <div className={cn("border-b border-border flex items-center justify-between shrink-0",
        collapsed ? "px-3 py-4" : "px-5 py-4")}>
        {!collapsed && (
          <div className="min-w-0">
            <h1 className="text-base font-bold tracking-tight truncate">{appName}</h1>
            <p className="text-[11px] text-muted-foreground mt-0.5">{appSubtitle}</p>
          </div>
        )}
        <button onClick={() => setSidebarOpen(false)} className="lg:hidden p-1 rounded-md hover:bg-accent">
          <X size={16} />
        </button>
      </div>

      {/* Navigation */}
      <div className={cn("flex-1 overflow-y-auto py-3", collapsed ? "px-2" : "px-3")}>
        {deviceItems.length > 0 && (
          <div className="mb-3">
            {!collapsed && (
              <p className="text-[11px] text-muted-foreground/70 font-medium uppercase tracking-wider px-3 mb-1.5">
                Device
              </p>
            )}
            <nav className="flex flex-col gap-0.5">{renderNav(deviceItems)}</nav>
          </div>
        )}

        {orgs.length > 0 && (
          <div className={cn("mb-3 pt-3 border-t border-border/50", collapsed ? "px-0" : "px-1")}>
            {collapsed ? (
              <div className="flex justify-center mb-1">
                <span className="text-[11px] font-semibold text-muted-foreground uppercase">
                  {activeOrgName.slice(0, 2)}
                </span>
              </div>
            ) : (
              <Select value={activeOrg} onValueChange={(v) => { onSelectOrg(v); setSidebarOpen(false); }}>
                <SelectTrigger className="h-8 text-xs"><SelectValue placeholder="Select org" /></SelectTrigger>
                <SelectContent>
                  {orgs.map((o) => (<SelectItem key={o.id} value={o.id}>{o.name}</SelectItem>))}
                </SelectContent>
              </Select>
            )}
          </div>
        )}

        {activeOrg && orgItems.length > 0 && (
          <div>
            {!collapsed && (
              <p className="text-[11px] text-muted-foreground/70 font-medium uppercase tracking-wider px-3 mb-1.5">
                {activeOrgName}
              </p>
            )}
            <nav className="flex flex-col gap-0.5">{renderNav(orgItems)}</nav>
          </div>
        )}

        {extraNavItems && extraNavItems.length > 0 && (
          <div className="mt-3 pt-3 border-t border-border/50">
            <nav className="flex flex-col gap-0.5">{renderNav(extraNavItems)}</nav>
          </div>
        )}
      </div>

      {/* Footer: node status + collapse toggle + theme */}
      <div className="shrink-0 border-t border-border py-3">
        {!collapsed && (
          <div className="flex items-center gap-2 px-4 mb-2">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse shrink-0" />
            <span className="text-[11px] text-muted-foreground truncate">{nodeId.slice(0, 18)}...</span>
          </div>
        )}
        <div className={cn("flex items-center", collapsed ? "flex-col gap-2 px-1" : "justify-between px-4")}>
          <ThemeToggle />
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="p-1.5 rounded-md hover:bg-accent text-muted-foreground transition-colors hidden lg:flex"
            title={collapsed ? "Expandir" : "Colapsar"}>
            <ChevronLeft size={16} className={cn("transition-transform", collapsed && "rotate-180")} />
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="min-h-screen bg-background">
      {/* Mobile overlay */}
      <Sheet open={sidebarOpen} onClose={() => setSidebarOpen(false)}>
        <div className="flex flex-col h-full w-72 bg-sidebar text-sidebar-foreground">
          {sidebarContent}
        </div>
      </Sheet>

      {/* Desktop sidebar: fixed, collapsible */}
      <aside
        className={cn(
          "hidden lg:flex flex-col fixed inset-y-0 left-0 z-30 bg-sidebar text-sidebar-foreground border-r border-border transition-all duration-200",
          collapsed ? "w-16" : "w-60",
        )}>
        {sidebarContent}
      </aside>

      {/* Main area */}
      <div className={cn("lg:transition-all lg:duration-200", collapsed ? "lg:pl-16" : "lg:pl-60")}>
        <header className="h-14 border-b border-border bg-background flex items-center px-4 gap-3 sticky top-0 z-20">
          <button onClick={() => setSidebarOpen(true)} className="lg:hidden p-1.5 -ml-1 rounded-md hover:bg-accent">
            <Menu size={18} />
          </button>
          <h2 className="text-sm font-semibold truncate">{currentLabel}</h2>
          <div className="ml-auto flex gap-2 items-center">
            {role && <span className="hidden sm:inline text-xs text-muted-foreground">{activeOrgName} · {role}</span>}
            {onRefresh && (
              <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onRefresh}><RefreshCw size={14} /></Button>
            )}
            {headerActions}
          </div>
        </header>
        <main>{children}</main>
      </div>
    </div>
  );
}

export { type NavItem, type OrgInfo };
