import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useIAQueue } from "../../hooks/useIAQueue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapper({ children }: { children: React.ReactNode }) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

describe("useIAQueue", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does nothing when orgId is undefined", () => {
    renderHook(() => useIAQueue(undefined), { wrapper: Wrapper });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("polls drizzle_execute for completed queries", async () => {
    (invoke as any).mockResolvedValue({ rows: [] });

    renderHook(() => useIAQueue("org-1"), { wrapper: Wrapper });

    // First call happens immediately
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("drizzle_execute", expect.objectContaining({
        sql: expect.stringContaining("ia_queries"),
        params: ["org-1"],
      }));
    });
  });

  it("shows toast for completed queries and marks them seen", async () => {
    (invoke as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === "drizzle_execute" && args.sql?.includes("ia_queries")) {
        // Return a completed query
        return Promise.resolve({
          rows: [["q1", "v1", "show me invoices"]],
        });
      }
      return Promise.resolve({ rows: [] });
    });

    renderHook(() => useIAQueue("org-1"), { wrapper: Wrapper });

    await waitFor(() => {
      expect(toast.success).toHaveBeenCalledWith(
        expect.stringContaining("show me invoices"),
        expect.any(Object),
      );
    });

    // Should mark as seen
    expect(invoke).toHaveBeenCalledWith("drizzle_execute", expect.objectContaining({
      sql: expect.stringContaining("UPDATE"),
    }));
  });
});
