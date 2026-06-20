import * as React from "react"
import { useNavigate, useLocation } from "react-router-dom"
import { RefreshCw } from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from "./ui/sidebar"
import { TooltipProvider } from "./ui/tooltip"
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from "./ui/select"
import { Button } from "./ui/button"
import { ThemeToggle } from "./ThemeToggle"
import { cn } from "../lib/utils"

export interface NavItem {
  href: string
  label: string
  icon: React.ComponentType<{ size?: number; className?: string }>
  badge?: number
  section?: "device" | "org"
}

export interface OrgInfo {
  id: string
  name: string
  role?: string
}

interface AppShellProps {
  appName: string
  appSubtitle: string
  nodeId: string
  orgs: OrgInfo[]
  activeOrg: string
  onSelectOrg: (id: string) => void
  onRefresh?: () => void
  navItems: NavItem[]
  extraNavItems?: NavItem[]
  headerActions?: React.ReactNode
  children: React.ReactNode
  onOpenCommand?: () => void
}

export function AppShell({
  appName,
  appSubtitle,
  nodeId,
  orgs,
  activeOrg,
  onSelectOrg,
  onRefresh,
  onOpenCommand,
  navItems,
  extraNavItems,
  headerActions,
  children,
}: AppShellProps) {
  const navigate = useNavigate()
  const location = useLocation()

  // Ctrl+K → Command Palette
  React.useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault()
        if (onOpenCommand) onOpenCommand()
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [onOpenCommand])

  const activeOrgName = orgs.find((o) => o.id === activeOrg)?.name ?? activeOrg
  const role = orgs.find((o) => o.id === activeOrg)?.role

  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(href + "/")

  const allItems = extraNavItems ? [...navItems, ...extraNavItems] : navItems
  const activeItem = allItems.find((i) => isActive(i.href))
  const currentLabel =
    activeItem?.label ??
    (location.pathname === "/inbox"
      ? "Inbox"
      : location.pathname === "/orgs"
        ? "Mis Orgs"
        : appName)

  return (
    <TooltipProvider>
      <SidebarProvider>
        <Sidebar collapsible="icon">
          {/* Header: branding + org selector */}
          <SidebarHeader>
            <div className="flex items-center gap-2 px-1 py-1">
              <img src="/favicon.svg" alt="" className="size-5 shrink-0" />
              <div className="flex flex-col min-w-0 group-data-[collapsible=icon]:hidden">
                <span className="text-sm font-semibold leading-tight truncate">{appName}</span>
                <span className="text-[11px] text-sidebar-foreground/60 truncate">{appSubtitle}</span>
              </div>
            </div>

            {orgs.length > 0 && (
              <div className="group-data-[collapsible=icon]:hidden px-1">
                <Select
                  value={activeOrg}
                  onValueChange={onSelectOrg}
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder="Select org" />
                  </SelectTrigger>
                  <SelectContent>
                    {orgs.map((o) => (
                      <SelectItem key={o.id} value={o.id}>
                        {o.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            {/* Collapsed mode: show 2-letter org abbreviation */}
            {orgs.length > 0 && activeOrgName && (
              <div className="hidden group-data-[collapsible=icon]:flex justify-center px-1">
                <span className="text-[11px] font-semibold text-sidebar-foreground/60 uppercase">
                  {activeOrgName.slice(0, 2)}
                </span>
              </div>
            )}
          </SidebarHeader>

          {/* Navigation */}
          <SidebarContent>
            <SidebarGroup>
              <SidebarMenu>
                {navItems.map((item) => (
                  <SidebarMenuItem key={item.href}>
                    <SidebarMenuButton
                      onClick={() => navigate(item.href)}
                      isActive={isActive(item.href)}
                      tooltip={item.label}
                    >
                      <item.icon size={16} />
                      <span>{item.label}</span>
                    </SidebarMenuButton>
                    {item.badge ? (
                      <SidebarMenuBadge>{item.badge}</SidebarMenuBadge>
                    ) : null}
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroup>

            {extraNavItems && extraNavItems.length > 0 && (
              <SidebarGroup>
                <SidebarMenu>
                  {extraNavItems.map((item) => (
                    <SidebarMenuItem key={item.href}>
                      <SidebarMenuButton
                        onClick={() => navigate(item.href)}
                        isActive={isActive(item.href)}
                        tooltip={item.label}
                      >
                        <item.icon size={16} />
                        <span>{item.label}</span>
                      </SidebarMenuButton>
                      {item.badge ? (
                        <SidebarMenuBadge>{item.badge}</SidebarMenuBadge>
                      ) : null}
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroup>
            )}
          </SidebarContent>

          {/* Footer: node status + theme toggle */}
          <SidebarFooter>
            <div className="flex items-center justify-between px-1 group-data-[collapsible=icon]:justify-center">
              <ThemeToggle />
              <div className="flex items-center gap-1.5 min-w-0 group-data-[collapsible=icon]:hidden">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0 animate-pulse" />
                <span className="text-[10px] text-sidebar-foreground/50 truncate font-mono">
                  {nodeId ? nodeId.slice(0, 14) + "…" : "—"}
                </span>
              </div>
            </div>
          </SidebarFooter>

          <SidebarRail />
        </Sidebar>

        {/* Main content area — SidebarInset handles the left offset automatically */}
        <SidebarInset>
          <header
            className={cn(
              "flex h-14 shrink-0 items-center gap-2 border-b bg-background px-4 sticky top-0 z-20",
              "transition-[width,height] ease-linear"
            )}
          >
            <SidebarTrigger className="-ml-1" />
            <h2 className="text-sm font-semibold truncate">{currentLabel}</h2>
            <div className="ml-auto flex gap-2 items-center">
              {role && (
                <span className="hidden sm:inline text-xs text-muted-foreground">
                  {activeOrgName} · {role}
                </span>
              )}
              {onRefresh && (
                <Button variant="ghost" size="icon" onClick={onRefresh}>
                  <RefreshCw size={14} />
                </Button>
              )}
              {headerActions}
            </div>
          </header>

          <div className="flex-1 overflow-auto">
            {children}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}

export { type NavItem as NavItemType, type OrgInfo as OrgInfoType }
