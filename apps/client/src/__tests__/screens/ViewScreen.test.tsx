import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { ViewScreen } from "../../screens/ViewScreen";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));
import { invoke } from "@tauri-apps/api/core";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapped({ org, viewId }: { org: string; viewId: string }) {
  return (
    <MemoryRouter initialEntries={[`/${org}/view/${viewId}`]}>
      <QueryClientProvider client={queryClient}>
        <Routes>
          <Route path="/:org/view/:viewId" element={<ViewScreen org={org} />} />
        </Routes>
      </QueryClientProvider>
    </MemoryRouter>
  );
}

describe("ViewScreen", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders loading state initially", () => {
    (invoke as any).mockImplementation(() => new Promise(() => {}));
    render(<Wrapped org="org-1" viewId="v1" />);
    expect(screen.getByText("Cargando vista...")).toBeDefined();
  });

  it("renders view page with entity info when loaded", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "drizzle_execute") {
        // First call: view definition
        // Second call: view data (needs view.sql to execute)
        return Promise.resolve({
          rows: [["SELECT * FROM customers", "customers", "[]", "root", '{"tags":[],"naturalLanguageQuery":"Clientes"}']],
        });
      }
      return Promise.resolve(null);
    });
    render(<Wrapped org="org-1" viewId="v1" />);
    await waitFor(() => {
      expect(screen.getByText("Clientes")).toBeDefined();
      expect(screen.getByText(/customers/i)).toBeDefined();
    });
  });

  it("shows not found message when view missing", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "drizzle_execute") return Promise.resolve({ rows: [] });
      return Promise.resolve(null);
    });
    render(<Wrapped org="org-1" viewId="nonexistent" />);
    await waitFor(() => {
      expect(screen.getByText("Vista no encontrada")).toBeDefined();
    });
  });
});
