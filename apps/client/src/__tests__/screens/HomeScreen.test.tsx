import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { HomeScreen } from "../../screens/HomeScreen";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
import { invoke } from "@tauri-apps/api/core";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapped({ org }: { org: string }) {
  return (
    <MemoryRouter>
      <QueryClientProvider client={queryClient}>
        <HomeScreen org={org} />
      </QueryClientProvider>
    </MemoryRouter>
  );
}

describe("HomeScreen", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
    (invoke as any).mockResolvedValue({ rows: [] });
  });

  it("renders input and send button", async () => {
    render(<Wrapped org="test-org" />);
    await waitFor(() => {
      expect(screen.getByPlaceholderText(/facturas/i)).toBeDefined();
    });
  });

  it("calls ai_chat when sending a query", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "drizzle_execute") return Promise.resolve({ rows: [] });
      if (cmd === "ai_chat") return Promise.resolve({ reply: "Here are the invoices", tool_calls: [], status: "Generated" });
      return Promise.resolve(null);
    });
    render(<Wrapped org="test-org" />);
    const input = await screen.findByPlaceholderText(/facturas/i) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "show me invoices" } });
    const sendButton = await screen.findByText("Enviar");
    fireEvent.click(sendButton);
    await vi.waitFor(() => {
      expect(invoke.mock.calls.some((c: any[]) => c[0] === "ai_chat")).toBe(true);
    }, { timeout: 5000 });
  });

  it("shows recent views when available", async () => {
    (invoke as any).mockResolvedValue({
      rows: [
        ["v1", "SELECT * FROM customers", "customers", '{}', "1000", ""],
      ],
    });
    render(<Wrapped org="test-org" />);
    await waitFor(() => {
      expect(screen.getByText("customers")).toBeDefined();
      expect(screen.getByText(/SELECT \* FROM customers/i)).toBeDefined();
    });
  });
});
