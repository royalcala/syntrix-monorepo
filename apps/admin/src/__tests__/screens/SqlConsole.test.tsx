import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SqlConsole } from "../../screens/SqlConsole";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("SqlConsole", () => {
  beforeEach(() => vi.clearAllMocks());

  it("runs the default query on mount and renders results", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        return Promise.resolve({
          columns: ["id", "name"],
          rows: [["c1", "Alice"]],
          truncated: false,
        });
      }
      return Promise.resolve(null);
    });

    render(<SqlConsole initialQuery="SELECT id, name FROM customers" />);

    await waitFor(() => {
      expect(screen.getByText("Alice")).toBeDefined();
    });
    expect(invoke).toHaveBeenCalledWith("run_sql", expect.objectContaining({ query: "SELECT id, name FROM customers" }));
  });

  it("shows an error message when run_sql rejects", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") return Promise.reject(new Error("only SELECT statements are allowed"));
      return Promise.resolve(null);
    });

    render(<SqlConsole initialQuery="DELETE FROM customers" />);

    await waitFor(() => {
      expect(screen.getByText(/only select statements/i)).toBeDefined();
    });
  });

  it("loads a saved view into the editor and runs it", async () => {
    const savedViews = [{ id: "v1", name: "My View", sql_query: "SELECT 1", created_at: 0, updated_at: 0 }];
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve(savedViews);
      if (cmd === "run_sql") return Promise.resolve({ columns: ["1"], rows: [[1]], truncated: false });
      return Promise.resolve(null);
    });

    render(<SqlConsole />);

    await waitFor(() => {
      expect(screen.getByText("My View")).toBeDefined();
    });

    fireEvent.click(screen.getByText("My View"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_sql", expect.objectContaining({ query: "SELECT 1" }));
    });
  });

  it("creates a saved view from the current query", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") return Promise.resolve({ columns: [], rows: [], truncated: false });
      if (cmd === "create_saved_view") return Promise.resolve({ id: "v2", name: "New View", sql_query: "SELECT 1", created_at: 0, updated_at: 0 });
      return Promise.resolve(null);
    });

    render(<SqlConsole initialQuery="SELECT 1" />);

    const nameInput = await screen.findByPlaceholderText(/nombre de vista/i);
    fireEvent.change(nameInput, { target: { value: "New View" } });
    fireEvent.click(screen.getByText("Guardar"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_saved_view", { name: "New View", sqlQuery: "SELECT 1" });
    });
  });

  it("disables Next when the page is exactly full but the backend reports no more rows (truncated=false)", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        // Exactly 100 rows (== PAGE_SIZE) but the backend says there's nothing beyond this
        // page. The old `rows.length < PAGE_SIZE` check would leave "Next" enabled here.
        const rows = Array.from({ length: 100 }, (_, i) => [`c${i}`]);
        return Promise.resolve({ columns: ["id"], rows, truncated: false });
      }
      return Promise.resolve(null);
    });

    render(<SqlConsole initialQuery="SELECT id FROM customers" />);

    const nextButton = await screen.findByText("Siguiente");
    await waitFor(() => {
      expect((nextButton as HTMLButtonElement).disabled).toBe(true);
    });
  });

  it("enables Next when the page is full and the backend reports more rows (truncated=true)", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        const rows = Array.from({ length: 100 }, (_, i) => [`c${i}`]);
        return Promise.resolve({ columns: ["id"], rows, truncated: true });
      }
      return Promise.resolve(null);
    });

    render(<SqlConsole initialQuery="SELECT id FROM customers" />);

    const nextButton = await screen.findByText("Siguiente");
    await waitFor(() => {
      expect((nextButton as HTMLButtonElement).disabled).toBe(false);
    });
  });
});
