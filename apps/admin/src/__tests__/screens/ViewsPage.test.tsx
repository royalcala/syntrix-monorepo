import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { ViewsPage } from "../../screens/ViewsPage";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapped({ org }: { org: string }) {
  return (
    <MemoryRouter>
      <QueryClientProvider client={queryClient}>
        <ViewsPage org={org} />
      </QueryClientProvider>
    </MemoryRouter>
  );
}

describe("ViewsPage", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("shows empty state when no views exist", async () => {
    (invoke as any).mockResolvedValue({ rows: [] });
    render(<Wrapped org="test-org" />);
    await waitFor(() => {
      expect(screen.getByText(/No hay vistas guardadas/i)).toBeDefined();
    });
  });

  it("renders list of views when data is loaded", async () => {
    (invoke as any).mockResolvedValue({
      rows: [
        ["v1", "SELECT * FROM customers", "customers", "admin", "clientes", "{}", "1000"],
        ["v2", "SELECT * FROM invoices", "invoices", "admin", "", "{}", "1001"],
      ],
    });
    render(<Wrapped org="test-org" />);
    await waitFor(() => {
      expect(screen.getByText("customers")).toBeDefined();
      expect(screen.getByText("invoices")).toBeDefined();
    });
  });

  it("calls drizzle_execute with correct org filter", async () => {
    (invoke as any).mockResolvedValue({ rows: [] });
    render(<Wrapped org="acme" />);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("drizzle_execute", expect.objectContaining({
        params: ["acme"],
      }));
    });
  });
});
