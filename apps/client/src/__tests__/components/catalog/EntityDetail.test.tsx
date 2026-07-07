import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { EntityDetailRenderer } from "@syntrix/ui/components/catalog/EntityDetailComponent";
import type { EntityDetailComponent } from "@syntrix/ui/components/catalog/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

const detailDef: EntityDetailComponent = {
  id: "d1",
  type: "entityDetail",
  entity: "customers",
  fields: [
    { key: "name", label: "Name", type: "text" },
    { key: "email", label: "Email", type: "text" },
  ],
  relations: [],
};

describe("EntityDetailRenderer", () => {
  it("shows placeholder when no data", () => {
    render(
      <MemoryRouter>
        <EntityDetailRenderer component={detailDef} data={[]} />
      </MemoryRouter>
    );
    expect(screen.getByText(/Selecciona un registro/i)).toBeDefined();
  });

  it("renders field labels and values from first row", () => {
    const data = [{ name: "Alice", email: "alice@test.com" }];
    render(
      <MemoryRouter>
        <EntityDetailRenderer component={detailDef} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Name")).toBeDefined();
    expect(screen.getByText("Alice")).toBeDefined();
    expect(screen.getByText("Email")).toBeDefined();
    expect(screen.getByText("alice@test.com")).toBeDefined();
  });

  it("renders selected row when selectedId matches doc_id", () => {
    const data = [
      { doc_id: "a", name: "Alice" },
      { doc_id: "b", name: "Bob" },
    ];
    render(
      <MemoryRouter>
        <EntityDetailRenderer component={detailDef} data={data} selectedId="b" />
      </MemoryRouter>
    );
    expect(screen.getByText("Bob")).toBeDefined();
  });

  it("renders em-dash for null values", () => {
    const data = [{ name: null, email: "a@b.com" }];
    render(
      <MemoryRouter>
        <EntityDetailRenderer component={detailDef} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("—")).toBeDefined();
  });

  it("renders relations section when present", () => {
    const defWithRel: EntityDetailComponent = {
      ...detailDef,
      relations: [{ entity: "invoices", label: "Invoices", fkField: "customer_id" }],
    };
    const data = [{ name: "Alice", customer_id: "c1" }];
    render(
      <MemoryRouter>
        <EntityDetailRenderer component={defWithRel} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("Relaciones")).toBeDefined();
    expect(screen.getByText("Invoices")).toBeDefined();
  });
});
