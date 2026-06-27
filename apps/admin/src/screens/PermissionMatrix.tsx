import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type EntitySchema = {
  name: string;
  version: number;
  namespace: string;
  fields: { name: string; field_type: string; indexed: boolean; searchable: boolean; sort_key: boolean; relation: { target: string } | null }[];
  indexes: { name: string; fields: string[] }[];
};

type Props = {
  canOpen: string[];
  canWrite: string[];
  onChange: (canOpen: string[], canWrite: string[]) => void;
};

const NAMESPACE_LABELS: Record<string, string> = {
  catalogs: "Catálogos",
  operational: "Operacional",
  payroll: "Nómina",
};

export function PermissionMatrix({ canOpen, canWrite, onChange }: Props) {
  const [schemas, setSchemas] = useState<EntitySchema[]>([]);
  const [search, setSearch] = useState("");
  const [hasAll, setHasAll] = useState(() => canOpen.includes("*"));

  useEffect(() => {
    invoke<EntitySchema[]>("get_schema_registry").then(setSchemas).catch(console.error);
  }, []);

  const toggleAll = useCallback(() => {
    if (hasAll) {
      onChange([], []);
      setHasAll(false);
    } else {
      onChange(["*"], ["*"]);
      setHasAll(true);
    }
  }, [hasAll, onChange]);

  // Group schemas by namespace
  const grouped = schemas.reduce<Record<string, EntitySchema[]>>((acc, s) => {
    const ns = s.namespace || "other";
    if (!acc[ns]) acc[ns] = [];
    acc[ns].push(s);
    return acc;
  }, {});

  const filtered = (entities: EntitySchema[]) => {
    if (!search) return entities;
    const q = search.toLowerCase();
    return entities.filter((e) => e.name.includes(q));
  };

  const toggleNamespace = (ns: string, col: "read" | "write") => {
    if (hasAll) return;
    const list = col === "read" ? canOpen : canWrite;
    const entities = schemas.filter((s) => (s.namespace || "other") === ns);
    const allSelected = entities.every((e) => list.includes(e.name));
    const updated = [...list];
    if (allSelected) {
      // Deselect all in namespace
      entities.forEach((e) => {
        const idx = updated.indexOf(e.name);
        if (idx >= 0) updated.splice(idx, 1);
      });
    } else {
      // Select all in namespace
      entities.forEach((e) => {
        if (!updated.includes(e.name)) updated.push(e.name);
      });
    }
    if (col === "read") {
      onChange(updated, canWrite);
    } else {
      onChange(canOpen, updated);
    }
  };

  const toggleEntity = (entity: string, col: "read" | "write") => {
    if (hasAll) return;
    const list = col === "read" ? [...canOpen] : [...canWrite];
    const idx = list.indexOf(entity);
    if (idx >= 0) list.splice(idx, 1);
    else list.push(entity);
    if (col === "read") {
      onChange(list, canWrite);
    } else {
      onChange(canOpen, list);
    }
  };

  const namespaceAllSelected = (ns: string, col: "read" | "write") => {
    if (hasAll) return true;
    const list = col === "read" ? canOpen : canWrite;
    const entities = schemas.filter((s) => (s.namespace || "other") === ns);
    return entities.length > 0 && entities.every((e) => list.includes(e.name));
  };

  const namespaceSomeSelected = (ns: string, col: "read" | "write") => {
    if (hasAll) return true;
    const list = col === "read" ? canOpen : canWrite;
    const entities = schemas.filter((s) => (s.namespace || "other") === ns);
    return entities.some((e) => list.includes(e.name));
  };

  if (schemas.length === 0) return <div className="text-sm text-muted-foreground p-4">Cargando esquemas…</div>;

  return (
    <div className="space-y-4">
      {/* All-access toggle */}
      <label className="flex items-center gap-3 p-3 bg-amber-50 dark:bg-amber-950/20 rounded-lg border cursor-pointer hover:bg-amber-100/50 dark:hover:bg-amber-950/30 transition-colors">
        <input
          type="checkbox"
          checked={hasAll}
          onChange={toggleAll}
          className="h-4 w-4 rounded border-gray-300"
        />
        <div>
          <span className="text-sm font-medium">Acceso total ({"*"})</span>
          <p className="text-xs text-muted-foreground">Otorga acceso de lectura y escritura a todas las entidades y namespaces</p>
        </div>
      </label>

      {/* Search */}
      <input
        className="h-9 w-full rounded-md border px-3 py-1.5 text-sm bg-background"
        placeholder="Buscar entidad…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />

      {/* Permission matrix */}
      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted/50">
            <tr>
              <th className="text-left px-3 py-2 font-medium w-[60%]">Entidad</th>
              <th className="text-center px-3 py-2 font-medium">Leer</th>
              <th className="text-center px-3 py-2 font-medium">Escribir</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(grouped).map(([ns, entities]) => {
              const visible = filtered(entities);
              if (visible.length === 0 && search) return null;
              return (
                <>
                  {/* Namespace header row */}
                  <tr key={ns} className="bg-muted/20 border-t">
                    <td className="px-3 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider" colSpan={3}>
                      {NAMESPACE_LABELS[ns] || ns}
                    </td>
                  </tr>
                  {/* Namespace select-all row */}
                  <tr key={`${ns}-all`} className="bg-muted/10">
                    <td className="px-3 py-1 text-xs text-muted-foreground pl-6">Seleccionar todo</td>
                    <td className="text-center py-1">
                      <input
                        type="checkbox"
                        checked={namespaceAllSelected(ns, "read")}
                        ref={(el) => { if (el) el.indeterminate = namespaceSomeSelected(ns, "read") && !namespaceAllSelected(ns, "read"); }}
                        onChange={() => toggleNamespace(ns, "read")}
                        disabled={hasAll}
                        className="h-4 w-4"
                      />
                    </td>
                    <td className="text-center py-1">
                      <input
                        type="checkbox"
                        checked={namespaceAllSelected(ns, "write")}
                        ref={(el) => { if (el) el.indeterminate = namespaceSomeSelected(ns, "write") && !namespaceAllSelected(ns, "write"); }}
                        onChange={() => toggleNamespace(ns, "write")}
                        disabled={hasAll}
                        className="h-4 w-4"
                      />
                    </td>
                  </tr>
                  {/* Entity rows */}
                  {(search ? visible : entities).map((e) => (
                    <tr key={e.name} className="border-t border-muted/30 hover:bg-muted/20">
                      <td className="px-3 py-2 pl-8 text-sm capitalize">{e.name}</td>
                      <td className="text-center py-2">
                        <input
                          type="checkbox"
                          checked={hasAll || canOpen.includes(e.name)}
                          onChange={() => toggleEntity(e.name, "read")}
                          disabled={hasAll}
                          className="h-4 w-4"
                        />
                      </td>
                      <td className="text-center py-2">
                        <input
                          type="checkbox"
                          checked={hasAll || canWrite.includes(e.name)}
                          onChange={() => toggleEntity(e.name, "write")}
                          disabled={hasAll}
                          className="h-4 w-4"
                        />
                      </td>
                    </tr>
                  ))}
                </>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Hidden array display for debugging / reference */}
      <details className="text-xs text-muted-foreground">
        <summary className="cursor-pointer hover:text-foreground">Ver valores guardados</summary>
        <pre className="mt-1 p-2 bg-muted/30 rounded-md overflow-x-auto">{JSON.stringify({ can_open: canOpen, can_write: canWrite }, null, 2)}</pre>
      </details>
    </div>
  );
}
