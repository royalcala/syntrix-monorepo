import type { ReactNode } from "react";

export function ColumnLayout({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-3">{children}</div>;
}

export function RowLayout({ children }: { children: ReactNode }) {
  return <div className="flex flex-row gap-3 flex-wrap">{children}</div>;
}

export function CardLayout({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div className="bg-card border border-border rounded-xl">
      {title && (
        <div className="px-4 py-3 border-b border-border">
          <p className="text-sm font-medium">{title}</p>
        </div>
      )}
      <div className="p-4">{children}</div>
    </div>
  );
}
