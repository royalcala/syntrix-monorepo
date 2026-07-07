import { DataTableRenderer } from "./DataTableComponent";
import { MetricCardRenderer } from "./MetricCardComponent";
import { EntityDetailRenderer } from "./EntityDetailComponent";
import { FormRenderer } from "./FormComponent";
import { ColumnLayout, RowLayout, CardLayout } from "./LayoutComponents";
import type { ComponentDef, ViewDefinition } from "./types";

interface Props {
  view: ViewDefinition;
  data: Record<string, unknown>[];
  selectedId?: string;
  onCreateSubmit?: (entity: string, data: Record<string, unknown>) => Promise<void>;
}

function renderComponent(
  def: ComponentDef,
  data: Record<string, unknown>[],
  componentMap: Map<string, ComponentDef>,
  selectedId?: string,
  onCreateSubmit?: (entity: string, data: Record<string, unknown>) => Promise<void>,
): React.ReactNode {
  switch (def.type) {
    case "dataTable":
      return <DataTableRenderer key={def.id} component={def} data={data} />;

    case "metricCard":
      return <MetricCardRenderer key={def.id} component={def} data={data} />;

    case "entityDetail":
      return <EntityDetailRenderer key={def.id} component={def} data={data} selectedId={selectedId} />;

    case "form":
      return (
        <FormRenderer
          key={def.id}
          component={def}
          initialData={data[0]}
          onSubmit={async (formData) => {
            if (onCreateSubmit) await onCreateSubmit(def.entity, formData);
          }}
        />
      );

    case "column": {
      const children = def.children
        .map(id => componentMap.get(id))
        .filter((c): c is ComponentDef => c !== undefined)
        .map(c => renderComponent(c, data, componentMap, selectedId, onCreateSubmit));
      return <ColumnLayout key={def.id}>{children}</ColumnLayout>;
    }

    case "row": {
      const children = def.children
        .map(id => componentMap.get(id))
        .filter((c): c is ComponentDef => c !== undefined)
        .map(c => renderComponent(c, data, componentMap, selectedId, onCreateSubmit));
      return <RowLayout key={def.id}>{children}</RowLayout>;
    }

    case "card": {
      const childDef = componentMap.get(def.child);
      if (!childDef) return null;
      return (
        <CardLayout key={def.id} title={def.title}>
          {renderComponent(childDef, data, componentMap, selectedId, onCreateSubmit)}
        </CardLayout>
      );
    }

    default:
      return null;
  }
}

export function ViewRenderer({ view, data, selectedId, onCreateSubmit }: Props) {
  const componentMap = new Map(view.components.map(c => [c.id, c]));
  const rootDef = componentMap.get(view.root);

  if (!rootDef) {
    return (
      <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
        Vista inválida: componente raíz "{view.root}" no encontrado
      </div>
    );
  }

  return <>{renderComponent(rootDef, data, componentMap, selectedId, onCreateSubmit)}</>;
}
