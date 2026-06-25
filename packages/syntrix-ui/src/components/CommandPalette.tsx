import { useState, useEffect, useCallback } from "react";
import { Search, Plus } from "lucide-react";
import { cn } from "../lib/utils";

export interface SearchResult {
  id: string;
  label: string;
  subtitle?: string;
  entity: string;
  entityLabel: string;
  icon?: React.ComponentType<{ className?: string }>;
}

export interface QuickAction {
  id: string;
  label: string;
  shortcut?: string;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  results: SearchResult[];
  actions: QuickAction[];
  onSelectResult: (result: SearchResult) => void;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  isLoading?: boolean;
}

export function CommandPalette({
  open, onClose, results, actions, searchQuery, onSearchChange, onSelectResult, isLoading,
}: CommandPaletteProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);

  const allItems = [
    ...results.map((r) => ({ type: "result" as const, ...r })),
    ...(searchQuery ? actions.map((a) => ({ type: "action" as const, ...a })) : []),
  ];

  useEffect(() => {
    setSelectedIndex(0);
  }, [searchQuery]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setSelectedIndex((i) => Math.min(i + 1, allItems.length - 1)); }
    if (e.key === "ArrowUp") { e.preventDefault(); setSelectedIndex((i) => Math.max(i - 1, 0)); }
    if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      const item = allItems[selectedIndex]!;
      if (item.type === "result" && "entity" in item) onSelectResult(item as SearchResult);
      if (item.type === "action" && "action" in item) (item as QuickAction).action();
      onClose();
    }
    if (e.key === "Escape") { onClose(); }
  }, [selectedIndex, allItems, onSelectResult, onClose]);

  useEffect(() => {
    if (open) window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, handleKeyDown]);

  if (!open) return null;

  // Group results by entity
  const grouped = results.reduce<Record<string, SearchResult[]>>((acc, r) => {
    (acc[r.entityLabel] ??= []).push(r);
    return acc;
  }, {});

  return (
    <div className="fixed inset-0 z-50">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="fixed top-[20%] left-1/2 -translate-x-1/2 w-full max-w-lg">
        <div className="bg-popover rounded-xl border shadow-2xl overflow-hidden">
          {/* Search input */}
          <div className="flex items-center gap-2 px-4 py-3 border-b">
            <Search className="w-4 h-4 text-muted-foreground shrink-0" />
            <input
              autoFocus
              className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
              placeholder="Buscar en todas las entidades..."
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
            />
            <kbd className="text-[10px] px-1.5 py-0.5 rounded border bg-muted text-muted-foreground font-mono">esc</kbd>
          </div>

          {/* Results */}
          <div className="max-h-80 overflow-y-auto p-2">
            {isLoading ? (
              <div className="text-center text-sm text-muted-foreground py-6">Buscando...</div>
            ) : results.length === 0 && searchQuery ? (
              <div className="text-center text-sm text-muted-foreground py-6">Sin resultados para "{searchQuery}"</div>
            ) : (
              <>
                {Object.entries(grouped).map(([entityLabel, items]) => (
                  <div key={entityLabel} className="mb-2">
                    <p className="text-[11px] text-muted-foreground font-medium px-2 py-1">{entityLabel}</p>
                    {items.map((item) => {
                       const globalIndex = results.indexOf(item);
                      return (
                        <button
                          key={item.id}
                          onClick={() => { onSelectResult(item); onClose(); }}
                          className={cn(
                            "flex items-center gap-3 w-full px-3 py-2 text-sm rounded-md transition-colors text-left",
                            selectedIndex === globalIndex ? "bg-accent text-accent-foreground" : "hover:bg-muted/50",
                          )}>
                          <span className="text-muted-foreground text-xs font-mono shrink-0 w-16 truncate">{item.id.slice(0, 12)}</span>
                          <span className="flex-1 truncate">{item.label}</span>
                          {item.subtitle && (
                            <span className="text-xs text-muted-foreground truncate max-w-32">{item.subtitle}</span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                ))}

                {searchQuery && actions.length > 0 && (
                  <div className="pt-2 border-t mt-2">
                    <p className="text-[11px] text-muted-foreground font-medium px-2 py-1">Acciones</p>
                    {actions.map((action, i) => {
                      const globalIndex = results.length + i;
                      return (
                        <button
                          key={action.id}
                          onClick={() => { action.action(); onClose(); }}
                          className={cn(
                            "flex items-center gap-3 w-full px-3 py-2 text-sm rounded-md transition-colors text-left",
                            selectedIndex === globalIndex ? "bg-accent text-accent-foreground" : "hover:bg-muted/50",
                          )}>
                          <Plus className="w-4 h-4 text-muted-foreground" />
                          <span className="flex-1">{action.label}</span>
                          {action.shortcut && (
                            <kbd className="text-[10px] px-1.5 py-0.5 rounded border bg-muted text-muted-foreground font-mono">{action.shortcut}</kbd>
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center gap-4 px-4 py-2 border-t text-[11px] text-muted-foreground">
            <span className="flex items-center gap-1"><kbd className="px-1 rounded border bg-muted font-mono text-[10px]">↑↓</kbd> Navegar</span>
            <span className="flex items-center gap-1"><kbd className="px-1 rounded border bg-muted font-mono text-[10px]">↵</kbd> Abrir</span>
            <span className="flex items-center gap-1"><kbd className="px-1 rounded border bg-muted font-mono text-[10px]">esc</kbd> Cerrar</span>
          </div>
        </div>
      </div>
    </div>
  );
}

