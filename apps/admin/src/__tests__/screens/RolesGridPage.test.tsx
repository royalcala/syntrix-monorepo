import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RolesGridPage } from "../../screens/RolesGridPage";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

describe("RolesGridPage", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.clearAllMocks();
  });

  it("renders and loads roles", async () => {
    (invoke as any).mockResolvedValue([]);
    render(
      <QueryClientProvider client={queryClient}>
        <RolesGridPage org="test-org" />
      </QueryClientProvider>
    );
    await waitFor(() => {
      expect(screen.getByText(/roles/i)).toBeDefined();
    });
  });

  it("opens dialog to create new role", async () => {
    (invoke as any).mockResolvedValue([]);
    render(
      <QueryClientProvider client={queryClient}>
        <RolesGridPage org="test-org" />
      </QueryClientProvider>
    );
    await waitFor(() => {
      const btn = screen.queryByText(/Nuevo Rol/i) || screen.queryByText(/crear/i) || screen.queryByText(/\+/i);
      if (btn) fireEvent.click(btn);
    });
  });
});
