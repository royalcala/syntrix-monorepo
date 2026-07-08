import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { EntityGrid } from "@syntrix/ui/components/EntityGrid";
import { customersEntity } from "../../entities/customers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const mockData = [
  { org_id: "org-1", doc_id: "c1", name: "Acme Corp", tax_id: "ACM123", address: null, phone: null, email: "contact@acme.com", change_time: null, node_id: "" },
];

describe("EntityGrid", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders data loaded via query_entity", async () => {
    (invoke as any).mockResolvedValue(mockData);

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

  it("calls query_entity for data fetching", async () => {
    (invoke as any).mockResolvedValue([]);

    render(
      <MemoryRouter>
        <QueryClientProvider client={queryClient}>
          <EntityGrid entity={customersEntity} orgId="org-1" role="admin" />
        </QueryClientProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("query_entity", expect.objectContaining({ entity: "customers" }));
    });
  });
});
