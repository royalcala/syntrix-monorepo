import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type FieldSchema = {
  name: string;
  field_type: string;
  indexed: boolean;
  searchable: boolean;
  sort_key: boolean;
  relation: { target: string; field: string } | null;
};

type IndexDef = {
  name: string;
  fields: string[];
};

type EntitySchema = {
  name: string;
  version: number;
  fields: FieldSchema[];
  indexes: IndexDef[];
};

export function SchemaExplorer() {
  const [schemas, setSchemas] = useState<EntitySchema[]>([]);
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    invoke<EntitySchema[]>("get_schema_registry").then(setSchemas).catch(console.error);
  }, []);

  const sel = schemas.find((s) => s.name === selected);

  return (
    <div className="flex h-full gap-4">
      {/* Sidebar: entity list */}
      <div className="w-64 shrink-0 space-y-1 overflow-y-auto border-r pr-3">
        <h3 className="text-sm font-semibold text-muted-foreground mb-2">Entidades</h3>
        {schemas.map((s) => (
          <button
            key={s.name}
            onClick={() => setSelected(s.name)}
            className={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
              selected === s.name
                ? "bg-primary/10 text-primary font-medium"
                : "hover:bg-muted"
            }`}
          >
            <div className="flex items-center gap-2">
              <span className="capitalize">{s.name}</span>
              <span className="text-xs text-muted-foreground ml-auto">v{s.version}</span>
            </div>
          </button>
        ))}
      </div>

      {/* Detail panel */}
      <div className="flex-1 overflow-y-auto">
        {!sel ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            Selecciona una entidad para ver su esquema
          </div>
        ) : (
          <div className="space-y-6">
            <div>
              <h2 className="text-xl font-semibold capitalize">{sel.name}</h2>
              <p className="text-sm text-muted-foreground">Versión del esquema: {sel.version}</p>
            </div>

            {/* Fields */}
            <div>
              <h3 className="text-sm font-semibold text-muted-foreground mb-2">Campos</h3>
              <div className="border rounded-lg overflow-hidden">
                <table className="w-full text-sm">
                  <thead className="bg-muted/50">
                    <tr>
                      <th className="text-left px-3 py-2 font-medium">Campo</th>
                      <th className="text-left px-3 py-2 font-medium">Tipo</th>
                      <th className="text-center px-3 py-2 font-medium">Indexado</th>
                      <th className="text-center px-3 py-2 font-medium">Buscable</th>
                      <th className="text-center px-3 py-2 font-medium">Orden</th>
                      <th className="text-left px-3 py-2 font-medium">Relación</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sel.fields.map((f) => (
                      <tr key={f.name} className="border-t">
                        <td className="px-3 py-2 font-mono text-xs">{f.name}</td>
                        <td className="px-3 py-2 text-xs">{f.field_type}</td>
                        <td className="px-3 py-2 text-center">{f.indexed ? "✅" : "—"}</td>
                        <td className="px-3 py-2 text-center">{f.searchable ? "✅" : "—"}</td>
                        <td className="px-3 py-2 text-center">{f.sort_key ? "🔑" : "—"}</td>
                        <td className="px-3 py-2 text-xs">
                          {f.relation ? (
                            <span className="text-blue-500">
                              → {f.relation.target}.{f.relation.field}
                            </span>
                          ) : (
                            "—"
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>

            {/* Indexes */}
            {sel.indexes.length > 0 && (
              <div>
                <h3 className="text-sm font-semibold text-muted-foreground mb-2">Índices compuestos</h3>
                <div className="space-y-1">
                  {sel.indexes.map((idx) => (
                    <div key={idx.name} className="flex items-center gap-2 text-sm px-3 py-1.5 bg-muted/30 rounded-md">
                      <span className="font-mono text-xs text-muted-foreground">{idx.name}</span>
                      <span className="text-muted-foreground">→</span>
                      <span className="font-mono text-xs">{idx.fields.join(", ")}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Relations graph */}
            {sel.fields.some((f) => f.relation) && (
              <div>
                <h3 className="text-sm font-semibold text-muted-foreground mb-2">Relaciones</h3>
                <div className="flex flex-wrap gap-2">
                  {sel.fields
                    .filter((f) => f.relation)
                    .map((f) => (
                      <div key={f.name} className="flex items-center gap-1.5 text-xs bg-blue-500/10 text-blue-600 dark:text-blue-400 px-3 py-1.5 rounded-full">
                        <span className="font-mono">{sel.name}</span>
                        <span>.{f.name}</span>
                        <span>→</span>
                        <span className="font-mono">{f.relation!.target}</span>
                        <span>.{f.relation!.field}</span>
                      </div>
                    ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
