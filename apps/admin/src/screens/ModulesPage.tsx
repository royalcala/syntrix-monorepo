import { Puzzle, Package, FileJson } from "lucide-react";
import { Button } from "@syntrix/ui/components/ui/button";

const templates = [
  {
    name: "ERP Básico",
    description: "Clientes, productos, facturas, órdenes, nómina. Chart of accounts estándar.",
    coming: "Fase B",
  },
  {
    name: "Familiar",
    description: "Gastos del hogar, despensa, tareas, recordatorios.",
    coming: "Fase B",
  },
  {
    name: "Comunidad",
    description: "Fondo vecinal, guardias, inventario comunitario.",
    coming: "Fase B",
  },
];

export function ModulesPage() {
  return (
    <div className="flex flex-col h-full p-6">
      <div className="mb-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-1">
          <Package size={20} />
          Módulos
        </h2>
        <p className="text-sm text-muted-foreground">
          Los módulos extienden Syntrix con nuevas entidades, vistas y reglas de validación.
          Disponible en Fase B de la plataforma.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
        {templates.map((t) => (
          <div
            key={t.name}
            className="border border-border rounded-xl p-4 bg-card relative overflow-hidden"
          >
            <div className="absolute top-2 right-2 text-[10px] font-mono px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
              {t.coming}
            </div>
            <div className="flex items-center gap-2 mb-2">
              <Puzzle size={18} className="text-primary" />
              <span className="font-medium text-sm">{t.name}</span>
            </div>
            <p className="text-xs text-muted-foreground">{t.description}</p>
          </div>
        ))}
      </div>

      <div className="border border-dashed border-border rounded-xl p-8 text-center">
        <FileJson size={32} className="mx-auto mb-3 text-muted-foreground" />
        <h3 className="text-sm font-medium mb-1">Módulos Activos</h3>
        <p className="text-xs text-muted-foreground mb-4">
          No hay módulos instalados. Los módulos se generarán con IA y se sincronizarán vía CDC.
        </p>
        <Button variant="outline" size="sm" disabled>
          Explorar Templates
        </Button>
      </div>
    </div>
  );
}
