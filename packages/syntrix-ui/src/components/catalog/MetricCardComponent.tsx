import type { MetricCardComponent } from "./types";

interface Props {
  component: MetricCardComponent;
  data: Record<string, unknown>[];
}

function formatValue(val: unknown, format?: string): string {
  if (val === null || val === undefined) return "—";
  const num = typeof val === "number" ? val : Number(val);
  if (isNaN(num)) return String(val);
  switch (format) {
    case "currency": return `$${num.toLocaleString("es-MX", { minimumFractionDigits: 2 })}`;
    case "percentage": return `${(num * 100).toFixed(1)}%`;
    default: return num.toLocaleString();
  }
}

export function MetricCardRenderer({ component, data }: Props) {
  const value = data.length > 0 ? data[0][component.valueKey] : null;
  return (
    <div className="bg-card border border-border rounded-xl p-4">
      <p className="text-xs text-muted-foreground mb-1">{component.label}</p>
      <p className="text-2xl font-semibold">{formatValue(value, component.format)}</p>
    </div>
  );
}
