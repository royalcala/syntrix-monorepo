import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type EntitySchema = {
  name: string;
  version: number;
  fields: { name: string; field_type: string; indexed: boolean; searchable: boolean; sort_key: boolean; relation: { target: string } | null }[];
  indexes: { name: string; fields: string[] }[];
};

type Props = {
  canOpen: string[];
  canWrite: string[];
  onChange: (canOpen: string[], canWrite: string[]) => void;
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

  const filtered = schemas.filter((e) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return e.name.includes(q);
  });

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
          <p className="text-xs text-muted-foreground">Otorga acceso de lectura y escritura a todas las entidades</p>
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
            {filtered.map((e) => (
              <tr key={e.name} className="border-t border-muted/30 hover:bg-muted/20">
                <td className="px-3 py-2 pl-3 text-sm capitalize">{e.name}</td>
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
