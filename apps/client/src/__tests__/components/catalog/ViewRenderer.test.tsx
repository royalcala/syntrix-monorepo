import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ViewRenderer } from "@syntrix/ui/components/catalog/ViewRenderer";
import type { ViewDefinition, ViewMeta } from "@syntrix/ui/components/catalog/types";

const defaultMeta: ViewMeta = {
  naturalLanguageQuery: "test",
  createdBy: "test",
  createdAt: 1000,
  usageCount: 0,
  tags: [],
};

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

describe("ViewRenderer", () => {
  it("renders invalid root error when root component not found", () => {
    const view: ViewDefinition = {
      id: "v1", orgId: "org-1",
      sql: "SELECT * FROM customers",
      entity: "customers",
      components: [],
      root: "nonexistent",
      meta: defaultMeta,
    };
    render(
      <MemoryRouter>
        <ViewRenderer view={view} data={[]} />
      </MemoryRouter>
    );
    expect(screen.getByText(/componente raíz.*nonexistent.*no encontrado/i)).toBeDefined();
  });

  it("renders adjacency list recursively: column > dataTable", () => {
    const view: ViewDefinition = {
      id: "v2", orgId: "org-1",
      sql: "SELECT name, amount FROM invoices",
      entity: "invoices",
      components: [
        { id: "root", type: "column", children: ["t1"] },
        { id: "t1", type: "dataTable", columns: [{ key: "name", label: "Name" }, { key: "amount", label: "Amount" }] },
      ],
      root: "root",
      meta: defaultMeta,
    };
    const data = [{ name: "Invoice A", amount: 500 }];
    render(
      <MemoryRouter>
        <ViewRenderer view={view} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Invoice A")).toBeDefined();
    expect(screen.getByText("500")).toBeDefined();
  });

  it("renders row layout with two children", () => {
    const view: ViewDefinition = {
      id: "v3", orgId: "org-1",
      sql: "SELECT * FROM metrics",
      entity: "metrics",
      components: [
        { id: "root", type: "row", children: ["m1", "m2"] },
        { id: "m1", type: "metricCard", label: "Revenue", valueKey: "rev" },
        { id: "m2", type: "metricCard", label: "Cost", valueKey: "cost" },
      ],
      root: "root",
      meta: defaultMeta,
    };
    const data = [{ rev: 1000, cost: 600 }];
    render(
      <MemoryRouter>
        <ViewRenderer view={view} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Revenue")).toBeDefined();
    expect(screen.getByText("Cost")).toBeDefined();
    expect(screen.getByText("1,000")).toBeDefined();
    // TODO: currency format assertion depends on locale support in test env
  });

  it("renders card layout wrapping a child component", () => {
    const view: ViewDefinition = {
      id: "v4", orgId: "org-1",
      sql: "SELECT * FROM items",
      entity: "items",
      components: [
        { id: "root", type: "card", title: "Items Card", child: "t1" },
        { id: "t1", type: "dataTable", columns: [{ key: "name", label: "Name" }] },
      ],
      root: "root",
      meta: defaultMeta,
    };
    render(
      <MemoryRouter>
        <ViewRenderer view={view} data={[{ name: "Item 1" }]} />
      </MemoryRouter>
    );
    expect(screen.getByText("Items Card")).toBeDefined();
    expect(screen.getByText("Item 1")).toBeDefined();
  });

  it("returns null for unknown component type", () => {
    const view: ViewDefinition = {
      id: "v5",
      sql: "SELECT 1",
      entity: "test",
      components: [
        { id: "root", type: "unknown" as any, children: [] },
      ],
      root: "root",
      meta: defaultMeta,
    };
    const { container } = render(
      <MemoryRouter>
        <ViewRenderer view={view} data={[]} />
      </MemoryRouter>
    );
    expect(container.textContent).toBe("");
  });
});
