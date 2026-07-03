import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { customersEntity } from "../../entities/customers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));
import { invoke } from "@tauri-apps/api/core";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

// Column order: org_id, doc_id, name, tax_id, address, phone, email, change_time, node_id
const CUSTOMER_COLUMNS = ["org_id", "doc_id", "name", "tax_id", "address", "phone", "email", "change_time", "node_id"];

function mockDrizzleRows(rows: Record<string, string | null>[]) {
  return { rows: rows.map(r => CUSTOMER_COLUMNS.map(c => r[c] ?? null)) };
}

describe("EntityGrid", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders data loaded via Drizzle Proxy", async () => {
    const mockData = [
      { org_id: "org-1", doc_id: "c1", name: "Acme Corp", tax_id: "ACM123", address: null, phone: null, email: "contact@acme.com", change_time: null, node_id: "" }
    ];

    (invoke as any).mockResolvedValue(mockDrizzleRows(mockData));

    render(
      <MemoryRouter>
        <QueryClientProvider client={queryClient}>
          <EntityGrid entity={customersEntity} orgId="org-1" role="admin" />
        </QueryClientProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("Acme Corp")).toBeDefined();
    });
  });

  it("calls drizzle_execute for data fetching", async () => {
    (invoke as any).mockResolvedValue(mockDrizzleRows([]));

    render(
      <MemoryRouter>
        <QueryClientProvider client={queryClient}>
          <EntityGrid entity={customersEntity} orgId="org-1" role="admin" />
        </QueryClientProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("drizzle_execute", expect.objectContaining({
        sql: expect.stringContaining("customers"),
      }));
    });
  });
});
