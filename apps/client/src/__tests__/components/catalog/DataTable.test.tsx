import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { DataTableRenderer } from "@syntrix/ui/components/catalog/DataTableComponent";
import type { DataTableComponent } from "@syntrix/ui/components/catalog/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

const tableDef: DataTableComponent = {
  id: "t1",
  type: "dataTable",
  columns: [
    { key: "name", label: "Name" },
    { key: "amount", label: "Amount" },
  ],
};

describe("DataTableRenderer", () => {
  it("renders column headers", () => {
    render(
      <MemoryRouter>
        <DataTableRenderer component={tableDef} data={[]} />
      </MemoryRouter>
    );
    expect(screen.getByText("Name")).toBeDefined();
    expect(screen.getByText("Amount")).toBeDefined();
  });

  it("renders data rows", () => {
    const data = [
      { name: "Alice", amount: 100 },
      { name: "Bob", amount: 200 },
    ];
    render(
      <MemoryRouter>
        <DataTableRenderer component={tableDef} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Alice")).toBeDefined();
    expect(screen.getByText("Bob")).toBeDefined();
    expect(screen.getByText("100")).toBeDefined();
  });

  it("renders em-dash for null values", () => {
    const data = [{ name: "Charlie", amount: null }];
    render(
      <MemoryRouter>
        <DataTableRenderer component={tableDef} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Charlie")).toBeDefined();
    expect(screen.getByText("—")).toBeDefined();
  });

  it("filters out internal columns (org_id, doc_id, change_time, node_id)", () => {
    const defWithInternal: DataTableComponent = {
      id: "t2",
      type: "dataTable",
      columns: [
        { key: "name", label: "Name" },
        { key: "org_id", label: "Org" },
        { key: "doc_id", label: "Doc" },
      ],
    };
    render(
      <MemoryRouter>
        <DataTableRenderer component={defWithInternal} data={[{ name: "X", org_id: "o1", doc_id: "d1" }]} />
      </MemoryRouter>
    );
    expect(screen.getByText("Name")).toBeDefined();
    expect(screen.queryByText("Org")).toBeNull();
    expect(screen.queryByText("Doc")).toBeNull();
    expect(screen.getByText("X")).toBeDefined();
  });
});
