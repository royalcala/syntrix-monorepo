import { useNavigate } from "react-router-dom";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import type { EntityDetailComponent } from "./types";

interface Props {
  component: EntityDetailComponent;
  data: Record<string, unknown>[];
  selectedId?: string;
}

export function EntityDetailRenderer({ component, data, selectedId }: Props) {
  const navigate = useNavigate();
  const row = selectedId
    ? data.find(r => r.doc_id === selectedId || r.id === selectedId)
    : data[0];

  if (!row) {
    return (
      <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
        Selecciona un registro para ver el detalle
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        {component.fields.map(field => {
          const val = row[field.key];
          const display = val === null || val === undefined ? "—" : String(val);
          return (
            <div key={field.key}>
              <label className="text-xs text-muted-foreground block mb-0.5">{field.label}</label>
              <p className="text-sm">{display}</p>
            </div>
          );
        })}
      </div>

      {component.relations.length > 0 && (
        <div className="border-t border-border pt-3 mt-3">
          <p className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">Relaciones</p>
          <div className="space-y-1">
            {component.relations.map(rel => {
              const fkValue = row[rel.fkField];
              return (
                <div key={rel.entity} className="flex items-center justify-between">
                  <span className="text-sm">{rel.label}</span>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => {
                      if (fkValue) navigate(`/${rel.entity}?${rel.fkField}=${fkValue}`);
                    }}
                    disabled={!fkValue}
                  >
                    <Badge variant="secondary" className="mr-1 text-[10px]">{rel.entity}</Badge>
                    Ver
                  </Button>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
