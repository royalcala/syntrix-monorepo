import { useState, useEffect } from "react";
import { X, ChevronLeft, ChevronRight, Check, Loader2 } from "lucide-react";
import { useForm } from "@tanstack/react-form";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from "./ui/select";
import { Switch } from "./ui/switch";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import type { EntityDefinition } from "../fields/registry";

interface DetailPanelProps {
  entity: EntityDefinition;
  row: Record<string, unknown>;
  role?: string;
  onClose: () => void;
  onNavigate?: (dir: number) => void;
  isCreate?: boolean;
  onSaveCreate?: (row: Record<string, unknown>) => Promise<void>;
  onSaveUpdate?: (recordId: string, changes: Record<string, unknown>) => Promise<void>;
  customActions?: (row: Record<string, unknown>) => React.ReactNode;
}

export function DetailPanel({ entity, row, role, onClose, onNavigate, isCreate, onSaveCreate, onSaveUpdate, customActions }: DetailPanelProps) {
  const [activeTab, setActiveTab] = useState(isCreate ? "data" : entity.detail.tabs[0]?.key ?? "data");
  const [editMode, setEditMode] = useState(!!isCreate);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const form = useForm({
    defaultValues: row as Record<string, unknown>,
    onSubmit: async ({ value }) => {
      try {
        // Asegurar que los campos no editables (como el ID) se incluyan en el payload final
        const finalValue = { ...row, ...value };
        if (isCreate && onSaveCreate) {
          await onSaveCreate(finalValue);
          toast.success(`${entity.label} creado`);
        } else if (!isCreate && onSaveUpdate) {
          const recordId = finalValue.id || (finalValue as any).node_id || (finalValue as any).name;
          await onSaveUpdate(recordId as string, finalValue);
          toast.success("Cambios guardados");
        } else {
          const eventType = isCreate ? `${entity.id}.created` : `${entity.id}.updated`;
          await invoke("commit_event", { eventType, payload: JSON.stringify(finalValue) });
          toast.success(isCreate ? `${entity.label} creado` : "Cambios guardados");
        }
        setEditMode(false);
        if (isCreate && onClose) onClose();
      } catch (error) {
        console.error("Save failed:", error);
        toast.error(error instanceof Error ? error.message : "Error al guardar los cambios");
      }
    },
  });

  useEffect(() => {
    if (row) {
      Object.entries(row).forEach(([k, v]) => {
        form.setFieldValue(k, v);
      });
    }
    setFieldErrors({});
    setEditMode(!!isCreate);
  }, [row, isCreate]);

  const validateField = (key: string, value: unknown): string => {
    const field = entity.fields.find((f) => f.key === key);
    if (!field) return "";
    if (field.type === "number" || field.type === "currency") {
      if (value === "" || value == null) return "";
      if (isNaN(Number(value))) return "Debe ser un número";
    }
    if (field.type === "email" && value && typeof value === "string" && !value.includes("@")) {
      return "Email inválido";
    }
    return "";
  };

  const handleFieldChange = (key: string, value: unknown) => {
    form.setFieldValue(key, value);
    const err = validateField(key, value);
    setFieldErrors((prev) => err ? { ...prev, [key]: err } : Object.fromEntries(Object.entries(prev).filter(([k]) => k !== key)));
  };

  const handleSubmit = () => {
    const newErrors: Record<string, string> = {};
    entity.fields.forEach((f) => {
      const val = form.getFieldValue(f.key);
      const err = validateField(f.key, val);
      if (err) newErrors[f.key] = err;
    });
    if (Object.keys(newErrors).length > 0) { setFieldErrors(newErrors); return; }
    form.handleSubmit();
  };

  const renderDataTab = () => (
    <div className="space-y-4 p-4">
      {entity.fields.map((field) => {
        if (field.permissions?.view && role && !field.permissions.view.includes(role)) return null;
        const value = form.getFieldValue(field.key);
        const error = fieldErrors[field.key];

        return (
          <div key={field.key} className="space-y-1.5">
            <label className="text-xs font-medium text-muted-foreground flex items-center gap-1">
              {field.label}
              {!field.editable && !isCreate && <span className="text-[10px] text-muted-foreground/50">(automático)</span>}
            </label>

            {editMode && (field.editable || isCreate) ? (
              <div>
                {field.type === "status" || field.type === "select" ? (
                  <Select value={String(value ?? "")} onValueChange={(v) => handleFieldChange(field.key, v)}>
                    <SelectTrigger className={error ? "border-destructive" : ""}><SelectValue /></SelectTrigger>
                    <SelectContent>
                      {(field.options ?? (
                        field.type === "status" ? [
                          { label: "Borrador", value: "draft" }, { label: "Abierta", value: "open" },
                          { label: "Pagada", value: "paid" }, { label: "Cancelada", value: "cancelled" },
                        ] : []
                      )).map((opt) => (<SelectItem key={opt.value} value={opt.value}>{opt.label}</SelectItem>))}
                    </SelectContent>
                  </Select>
                ) : field.type === "multi-select" ? (
                  <div className="flex flex-wrap gap-2">
                    {(field.options ?? []).map((opt) => {
                      const arr = Array.isArray(value) ? value : [];
                      const isSelected = arr.includes(opt.value);
                      return (
                        <button
                          key={opt.value}
                          type="button"
                          onClick={(e) => {
                            e.preventDefault();
                            handleFieldChange(field.key, isSelected ? arr.filter((v) => v !== opt.value) : [...arr, opt.value]);
                          }}
                          className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                            isSelected ? "bg-primary text-primary-foreground border-primary" : "bg-background text-muted-foreground border-input hover:bg-accent hover:text-accent-foreground"
                          }`}
                        >
                          {opt.label}
                        </button>
                      );
                    })}
                  </div>
                ) : field.type === "boolean" ? (
                  <Switch checked={!!value} onCheckedChange={(v) => handleFieldChange(field.key, v)} />
                ) : field.type === "date" ? (
                  <Input type="date" value={String(value ?? "").slice(0, 10)}
                    onChange={(e) => handleFieldChange(field.key, e.target.value)}
                    className={error ? "border-destructive" : ""} />
                ) : (
                  <Input
                    type={field.type === "number" || field.type === "currency" ? "number" : "text"}
                    value={String(value ?? "")}
                    onChange={(e) => handleFieldChange(field.key, field.type === "number" || field.type === "currency" ? Number(e.target.value) : e.target.value)}
                    className={error ? "border-destructive" : ""}
                    placeholder={field.type === "currency" ? "0.00" : field.type === "number" ? "0" : ""} />
                )}
                {error && <p className="text-xs text-destructive mt-1">{error}</p>}
              </div>
            ) : (
              <div className="text-sm px-3 py-2 bg-muted/30 rounded-md min-h-[2.25rem] flex items-center">
                {field.type === "status" ? <StatusBadge status={String(value ?? "")} />
                : field.type === "boolean" ? (value ? <Check className="w-4 h-4 text-green-600" /> : <X className="w-4 h-4 text-muted-foreground/30" />)
                : field.type === "currency" ? `$${Number(value ?? 0).toFixed(2)}`
                : field.type === "multi-select" ? (
                  <div className="flex gap-1 flex-wrap">
                    {Array.isArray(value) && value.length > 0 ? value.map((item, idx) => {
                      const safeItem = typeof item === "object" && item !== null ? ((item as any).value || JSON.stringify(item)) : String(item);
                      const label = field.options?.find((o) => o.value === safeItem)?.label ?? safeItem;
                      return <span key={idx} className="px-2 py-0.5 rounded-full text-[10px] font-medium bg-secondary text-secondary-foreground">{label}</span>;
                    }) : <span className="text-muted-foreground/50">—</span>}
                  </div>
                )
                : String(value ?? "—")}
              </div>
            )}
          </div>
        );
      })}

      {editMode && (
        <div className="flex gap-2 pt-2">
          <Button size="sm" onClick={handleSubmit} disabled={form.state.isSubmitting}>
            {form.state.isSubmitting && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            {form.state.isSubmitting ? "Guardando..." : isCreate ? "Crear" : "Guardar"}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => {
            if (isCreate) { onClose(); } else { form.reset(); setEditMode(false); setFieldErrors({}); }
          }} disabled={form.state.isSubmitting}>
            Cancelar
          </Button>
        </div>
      )}
    </div>
  );

  const renderHistoryTab = () => (
    <div className="p-4 text-sm text-muted-foreground text-center">Historial de cambios — próximamente</div>
  );

  const primaryFieldValue = String(form.getFieldValue(entity.fields[0]?.key ?? "id") ?? "");

  return (
    <div className="w-[420px] border-l bg-background flex flex-col shrink-0">
      <div className="flex items-center justify-between px-4 py-3 border-b">
        <div className="flex items-center gap-1 min-w-0">
          {onNavigate && !isCreate && (
            <>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => onNavigate(-1)}><ChevronLeft className="w-4 h-4" /></Button>
              <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => onNavigate(1)}><ChevronRight className="w-4 h-4" /></Button>
            </>
          )}
          <span className="text-sm font-medium truncate">
            {isCreate ? `Nuevo ${entity.label.toLowerCase()}` : primaryFieldValue || "Sin título"}
          </span>
        </div>
        <div className="flex items-center gap-1">
          {customActions && customActions(row)}
          {!isCreate && entity.fields.some((f) => f.editable) && (
            <Button variant="ghost" size="sm" onClick={() => setEditMode(!editMode)}>{editMode ? "Ver" : "Editar"}</Button>
          )}
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClose}><X className="w-4 h-4" /></Button>
        </div>
      </div>
      {!isCreate && (
        <div className="flex border-b px-2">
          {entity.detail.tabs.map((tab) => (
            <button key={tab.key} onClick={() => setActiveTab(tab.key)}
              className={`px-3 py-2 text-sm border-b-2 transition-colors ${activeTab === tab.key ? "border-primary text-primary font-medium" : "border-transparent text-muted-foreground hover:text-foreground"}`}>
              {tab.label}
            </button>
          ))}
        </div>
      )}
      <div className="flex-1 overflow-auto">
        {activeTab === "data" && renderDataTab()}
        {activeTab === "history" && renderHistoryTab()}
        {activeTab !== "data" && activeTab !== "history" && (
          <div className="p-4 text-sm text-muted-foreground text-center">
            {entity.detail.tabs.find((t) => t.key === activeTab)?.label} — próximamente
          </div>
        )}
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    draft: "bg-muted text-muted-foreground", open: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
    paid: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300", cancelled: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
    pending: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
  };
  return <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${colors[status] ?? "bg-muted text-muted-foreground"}`}>{status}</span>;
}
