import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@syntrix/ui/components/ui/button";
import { Input } from "@syntrix/ui/components/ui/input";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@syntrix/ui/components/ui/dialog";
import { Plus } from "lucide-react";
import { PermissionMatrix } from "./PermissionMatrix";

type Role = {
  name: string;
  can_open: string[];
  can_write: string[];
};

export function RolesGridPage({ org }: { org: string }) {
  const queryClient = useQueryClient();
  const [roles, setRoles] = useState<Role[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingRole, setEditingRole] = useState<Role | null>(null); // null = creating new
  const [isOpen, setIsOpen] = useState(false);
  
  // Form state
  const [name, setName] = useState("");
  const [canOpen, setCanOpen] = useState<string[]>([]);
  const [canWrite, setCanWrite] = useState<string[]>([]);

  const loadRoles = useCallback(async () => {
    if (!org) return;
    setLoading(true);
    try {
      const list: Role[] = await invoke("list_roles", { org });
      setRoles(list);
    } catch (e) {
      console.error("Failed to load roles:", e);
    }
    setLoading(false);
  }, [org]);

  useEffect(() => {
    localStorage.setItem("syntrix_admin_org", org);
    loadRoles();
  }, [org, loadRoles]);

  const openCreate = () => {
    setEditingRole(null);
    setName("");
    setCanOpen([]);
    setCanWrite([]);
    setIsOpen(true);
  };

  const openEdit = (role: Role) => {
    setEditingRole(role);
    setName(role.name);
    setCanOpen(role.can_open || []);
    setCanWrite(role.can_write || []);
    setIsOpen(true);
  };

  const handleSave = async () => {
    if (!name.trim()) return;
    try {
      if (editingRole) {
        // Update existing role
        await invoke("update_role", {
          org,
          key: editingRole.name,
          changes: { can_open: canOpen, can_write: canWrite },
        });
      } else {
        // Create new role
        await invoke("create_role", {
          org,
          name: name.trim(),
          canOpen,
          canWrite,
        });
      }
      setIsOpen(false);
      await loadRoles();
      queryClient.invalidateQueries({ queryKey: ["entity", "roles", org] });
    } catch (e) {
      console.error("Failed to save role:", e);
      alert("Error al guardar el rol");
    }
  };

  if (loading) return <div className="p-4 text-muted-foreground">Cargando roles...</div>;

  return (
    <div className="h-full flex flex-col">
      {/* Header with Create Button */}
      <div className="flex justify-end p-4 border-b">
        <Button size="sm" onClick={openCreate}>
          <Plus className="w-4 h-4 mr-2" /> Nuevo Rol
        </Button>
      </div>

      {/* Roles List */}
      <div className="flex-1 overflow-auto">
        <table className="w-full text-sm">
          <thead className="bg-muted/50 sticky top-0">
            <tr>
              <th className="text-left px-4 py-3 font-medium">Nombre</th>
              <th className="text-left px-4 py-3 font-medium">Leer (Entidades)</th>
              <th className="text-left px-4 py-3 font-medium">Escribir (Entidades)</th>
              <th className="text-right px-4 py-3 font-medium">Acciones</th>
            </tr>
          </thead>
          <tbody>
            {roles.length === 0 ? (
              <tr>
                <td colSpan={4} className="text-center py-8 text-muted-foreground">
                  No hay roles definidos
                </td>
              </tr>
            ) : (
              roles.map((role) => (
                <tr key={role.name} className="border-t hover:bg-muted/20">
                  <td className="px-4 py-3 font-medium">{role.name}</td>
                  <td className="px-4 py-3 text-muted-foreground text-xs">
                    {role.can_open?.includes("*") ? (
                      <span className="text-green-600 font-medium">Todas (*)</span>
                    ) : (
                      role.can_open?.join(", ") || "Ninguna"
                    )}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground text-xs">
                    {role.can_write?.includes("*") ? (
                      <span className="text-green-600 font-medium">Todas (*)</span>
                    ) : (
                      role.can_write?.join(", ") || "Ninguna"
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <Button variant="ghost" size="sm" onClick={() => openEdit(role)}>
                      Editar
                    </Button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Edit/Create Modal */}
      <Dialog open={isOpen} onOpenChange={setIsOpen}>
        <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>
              {editingRole ? `Editar Rol: ${editingRole.name}` : "Nuevo Rol"}
            </DialogTitle>
          </DialogHeader>

          <div className="flex-1 overflow-auto py-4 space-y-6">
            {/* Role Name Input (only for new roles) */}
            {!editingRole && (
              <div className="space-y-2">
                <label className="text-sm font-medium">Nombre del Rol</label>
                <Input 
                  value={name} 
                  onChange={(e) => setName(e.target.value)} 
                  placeholder="ej. ventas, contabilidad, admin"
                  autoFocus
                />
              </div>
            )}

            {/* Permission Matrix */}
            <div className="space-y-2">
              <label className="text-sm font-medium">Permisos de Acceso</label>
              <PermissionMatrix 
                canOpen={canOpen} 
                canWrite={canWrite} 
                onChange={(newOpen, newWrite) => {
                  setCanOpen(newOpen);
                  setCanWrite(newWrite);
                }} 
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setIsOpen(false)}>Cancelar</Button>
            <Button onClick={handleSave} disabled={!name.trim() && !editingRole}>
              {editingRole ? "Guardar Cambios" : "Crear Rol"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
