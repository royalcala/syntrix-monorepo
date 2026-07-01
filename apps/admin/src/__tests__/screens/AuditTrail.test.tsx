import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { AuditTrail } from "../../screens/AuditTrail";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("AuditTrail", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders audit trail with empty state", async () => {
    (invoke as any).mockResolvedValue([]);
    render(<AuditTrail />);
    await waitFor(() => {
      expect(screen.getByText(/auditoría/i)).toBeDefined();
    });
  });

  it("renders audit entries from invoke", async () => {
    const mockEntries = [
      { key: "evt1", event_type: "customer.created", hlc_ts: 1000, schema_version: 1, entity: "customers", doc_id: "c1", payload: {} },
    ];
    (invoke as any).mockResolvedValue(mockEntries);
    render(<AuditTrail />);
    await waitFor(() => {
      expect(screen.getByText("customer.created")).toBeDefined();
    });
  });

  it("handles invoke error gracefully", async () => {
    (invoke as any).mockRejectedValue(new Error("db error"));
    render(<AuditTrail />);
    await waitFor(() => {
      expect(screen.getByText(/auditoría/i)).toBeDefined();
    });
  });
});
