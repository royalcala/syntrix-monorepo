import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Setup } from "../../screens/Setup";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("Setup", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders node ID and join form", () => {
    render(<Setup nodeId="test-node-id" onJoined={() => {}} />);
    expect(screen.getByText("test-node-id")).toBeDefined();
    expect(screen.getByText(/Syntrix/)).toBeDefined();
  });

  it("calls join_org on submit and calls onJoined", async () => {
    const onJoined = vi.fn();
    (invoke as any).mockResolvedValue(undefined);
    render(<Setup nodeId="test-node" onJoined={onJoined} />);
    const textarea = screen.getByPlaceholderText(/Pega el ticket/i);
    fireEvent.change(textarea, { target: { value: "invite-ticket" } });
    fireEvent.click(screen.getByText("Unirse"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("join_org", { ticketStr: "invite-ticket" });
      expect(onJoined).toHaveBeenCalled();
    });
  });

  it("shows error when join_org fails", async () => {
    (invoke as any).mockRejectedValue(new Error("invalid ticket"));
    render(<Setup nodeId="test-node" onJoined={() => {}} />);
    fireEvent.change(screen.getByPlaceholderText(/Pega el ticket/i), { target: { value: "bad-ticket" } });
    fireEvent.click(screen.getByText("Unirse"));
    await waitFor(() => {
      expect(screen.getByText(/invalid ticket/)).toBeDefined();
    });
  });
});
