import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

describe("EntityGrid Editing Reactivity", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("should update table row when edit is saved and refetch queries", async () => {
    // 1. Mock the backend responses
    const mockData = [
      { id: "c1", name: "Acme Corp", email: "contact@acme.com", tax_id: "ACM123" }
    ];

    (invoke as any).mockImplementation((cmd: string, _args: any) => {
      if (cmd === "query_entity") return Promise.resolve(mockData);
      if (cmd === "commit_event") return Promise.resolve("ok");
      return Promise.resolve();
    });

    // 2. Render the grid
    render(
      <MemoryRouter>
        <QueryClientProvider client={queryClient}>
          <EntityGrid entity={customersEntity} orgId="org-1" role="admin" />
        </QueryClientProvider>
      </MemoryRouter>
    );

    // 3. Wait for data to load
    const cell = await screen.findByText("Acme Corp");
    const row = cell.closest("tr");
    expect(row).toBeDefined();

    // 4. Click the row (event will bubble to the row)
    fireEvent.click(row!);
    
    // 5. Wait for panel to open and click the "Editar" button to enter editMode
    const editButtons = await screen.findAllByText("Editar");
    fireEvent.click(editButtons[0]);

    // 6. Find the name input
    const nameInput = await screen.findByDisplayValue("Acme Corp");
    
    // 7. Edit the name
    fireEvent.change(nameInput, { target: { value: "Acme Corporation Inc." } });

    // 8. Change the mock data to simulate the backend having the new data
    const updatedData = [
      { id: "c1", name: "Acme Corporation Inc.", email: "contact@acme.com", tax_id: "ACM123" }
    ];
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "query_entity") return Promise.resolve(updatedData);
      if (cmd === "commit_event") return Promise.resolve("ok");
      return Promise.resolve();
    });

    // 9. Click Save
    const saveButton = screen.getByText("Guardar");
    fireEvent.click(saveButton);

    // 10. Verify commit_event was called correctly WITH the id
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("commit_event", expect.objectContaining({
        eventType: "customers.updated",
        payload: expect.stringContaining('"id":"c1"')
      }));
    });

    // 10. Verify the table reflects the new name!
    // If reactivity is working, the table should now show "Acme Corporation Inc." instead of "Acme Corp".
    await waitFor(() => {
      expect(screen.queryAllByText("Acme Corporation Inc.").length).toBeGreaterThan(0);
    });
  });
});
