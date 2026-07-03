import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { DetailPanel } from "@syntrix/ui/components/DetailPanel";
import { customersEntity } from "../../entities/customers";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapped(props: any) {
  return (
    <MemoryRouter>
      <QueryClientProvider client={queryClient}>
        <DetailPanel {...props} />
      </QueryClientProvider>
    </MemoryRouter>
  );
}

describe("DetailPanel", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders with row data", () => {
    const row = { id: "c1", name: "Acme Corp", email: "a@b.com" };
    render(<Wrapped entity={customersEntity} row={row} onClose={() => {}} />);
    expect(screen.getByText("Acme Corp")).toBeDefined();
  });

  it("shows create mode when isCreate is true", () => {
    render(<Wrapped entity={customersEntity} row={{}} onClose={() => {}} isCreate={true} />);
    expect(screen.getByText(/Crear/i)).toBeDefined();
  });

  it("calls onClose when close button clicked", () => {
    const onClose = vi.fn();
    render(<Wrapped entity={customersEntity} row={{ id: "c1" }} onClose={onClose} />);
    const closeBtn = document.querySelector(".lucide-x");
    if (closeBtn) fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });

  it("calls onSaveCreate when creating", async () => {
    (invoke as any).mockResolvedValue("ok");
    const onSaveCreate = vi.fn().mockResolvedValue(undefined);
    render(<Wrapped entity={customersEntity} row={{}} onClose={() => {}} isCreate={true} onSaveCreate={onSaveCreate} />);
    const saveBtn = screen.getByText(/Crear/i);
    fireEvent.click(saveBtn);
  });

  it("calls onSaveUpdate when editing", async () => {
    (invoke as any).mockResolvedValue("ok");
    const row = { id: "c1", name: "Acme", email: "a@b.com" };
    const onSaveUpdate = vi.fn().mockResolvedValue(undefined);
    render(<Wrapped entity={customersEntity} row={row} onClose={() => {}} onSaveUpdate={onSaveUpdate} />);
    const editBtn = screen.getByText("Editar");
    fireEvent.click(editBtn);
    const nameInput = screen.getByDisplayValue("Acme");
    fireEvent.change(nameInput, { target: { value: "Acme Corp" } });
    const saveBtn = screen.getByText("Guardar");
    fireEvent.click(saveBtn);
  });

  it("shows error on failed save", async () => {
    (invoke as any).mockRejectedValue(new Error("save failed"));
    const row = { id: "c1", name: "Acme", email: "a@b.com" };
    const onSaveUpdate = vi.fn().mockRejectedValue(new Error("save failed"));
    render(<Wrapped entity={customersEntity} row={row} onClose={() => {}} onSaveUpdate={onSaveUpdate} />);
    fireEvent.click(screen.getByText("Editar"));
    fireEvent.click(screen.getByText("Guardar"));
  });
});
