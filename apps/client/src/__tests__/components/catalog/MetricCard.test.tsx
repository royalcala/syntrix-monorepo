import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { MetricCardRenderer } from "@syntrix/ui/components/catalog/MetricCardComponent";
import type { MetricCardComponent } from "@syntrix/ui/components/catalog/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

describe("MetricCardRenderer", () => {
  const cardDef: MetricCardComponent = {
    id: "m1",
    type: "metricCard",
    label: "Total Revenue",
    valueKey: "total",
    format: "currency",
  };

  it("renders label", () => {
    render(
      <MemoryRouter>
        <MetricCardRenderer component={cardDef} data={[]} />
      </MemoryRouter>
    );
    expect(screen.getByText("Total Revenue")).toBeDefined();
  });

  it("renders currency-formatted value from first row", () => {
    const data = [{ total: 1234.5 }];
    render(
      <MemoryRouter>
        <MetricCardRenderer component={cardDef} data={data} />
      </MemoryRouter>
    );
    expect(screen.getByText("$1,234.50")).toBeDefined();
  });

  it("renders percentage format", () => {
    const pctDef: MetricCardComponent = { id: "m2", type: "metricCard", label: "Growth", valueKey: "pct", format: "percentage" };
    render(
      <MemoryRouter>
        <MetricCardRenderer component={pctDef} data={[{ pct: 0.156 }]} />
      </MemoryRouter>
    );
    expect(screen.getByText("15.6%")).toBeDefined();
  });

  it("renders number format", () => {
    const numDef: MetricCardComponent = { id: "m3", type: "metricCard", label: "Count", valueKey: "n" };
    render(
      <MemoryRouter>
        <MetricCardRenderer component={numDef} data={[{ n: 42 }]} />
      </MemoryRouter>
    );
    expect(screen.getByText("42")).toBeDefined();
  });

  it("shows em-dash when data is empty", () => {
    render(
      <MemoryRouter>
        <MetricCardRenderer component={cardDef} data={[]} />
      </MemoryRouter>
    );
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(1);
  });
});
