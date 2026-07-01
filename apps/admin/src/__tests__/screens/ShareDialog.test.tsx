import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ShareDialog } from "../../screens/ShareDialog";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

Object.assign(navigator, {
  clipboard: { writeText: vi.fn() },
});

describe("ShareDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders share button", () => {
    render(<ShareDialog org="test-org" />);
    expect(screen.getByText("Share")).toBeDefined();
  });

  it("opens dialog on share click", async () => {
    (invoke as any).mockResolvedValue(["ticket-123"]);
    render(<ShareDialog org="test-org" />);
    fireEvent.click(screen.getByText("Share"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("share_org", { org: "test-org" });
      expect(screen.getByText(/Invite to test-org/)).toBeDefined();
    });
  });

  it("copies tickets to clipboard", async () => {
    (invoke as any).mockResolvedValue(["ticket-abc"]);
    render(<ShareDialog org="test-org" />);
    fireEvent.click(screen.getByText("Share"));
    await waitFor(() => {
      fireEvent.click(screen.getByText(/Copy/));
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ticket-abc");
    });
  });
});
