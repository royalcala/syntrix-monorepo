import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { DevicesGridPage } from "../../screens/DevicesGridPage";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

function Wrapped({ org }: { org: string }) {
  return (
    <QueryClientProvider client={queryClient}>
      <DevicesGridPage org={org} />
    </QueryClientProvider>
  );
}

describe("DevicesGridPage", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders loading state", () => {
    (invoke as any).mockResolvedValue([]);
    render(<Wrapped org="test-org" />);
    expect(screen.getByText(/dispositivos/i)).toBeDefined();
  });

  it("renders devices after loading", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_roles") return Promise.resolve([{ name: "admin", can_open: ["*"], can_write: ["*"] }]);
      return Promise.resolve([]);
    });
    render(<Wrapped org="test-org" />);
    await waitFor(() => {
      expect(screen.getByText("admin")).toBeDefined();
    });
  });
});
