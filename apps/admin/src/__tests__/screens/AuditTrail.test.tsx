import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { AuditTrail } from "../../screens/AuditTrail";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// AuditTrail is now a thin wrapper around the generic read-only SQL console (Fase 4, tarea
// 23): it seeds the query box with a SELECT over `event_log` and delegates to `run_sql` /
// `list_saved_views`, instead of a bespoke `audit_query` filter UI.
describe("AuditTrail", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders with an empty result set", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") return Promise.resolve({ columns: ["change_time", "entity"], rows: [], truncated: false });
      return Promise.resolve(null);
    });
    render(<AuditTrail org="acme" />);
    await waitFor(() => {
      expect(screen.getByText(/sin resultados/i)).toBeDefined();
    });
  });

  it("renders audit rows returned by run_sql", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        return Promise.resolve({
          columns: ["change_time", "entity", "change_type", "doc_id", "node_id", "row_image"],
          rows: [[1000, "customers", "insert", "c1", "nodeA", "{}"]],
          truncated: false,
        });
      }
      return Promise.resolve(null);
    });
    render(<AuditTrail org="acme" />);
    await waitFor(() => {
      expect(screen.getByText("customers")).toBeDefined();
      expect(screen.getByText("insert")).toBeDefined();
    });
  });

  it("scopes the seeded query to the given org", async () => {
    let capturedQuery = "";
    (invoke as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        capturedQuery = args?.query ?? "";
        return Promise.resolve({ columns: [], rows: [], truncated: false });
      }
      return Promise.resolve(null);
    });
    render(<AuditTrail org="acme" />);
    await waitFor(() => {
      expect(capturedQuery).toContain("acme");
      expect(capturedQuery).toContain("event_log");
    });
  });

  it("escapes single quotes in the org name before interpolating into the SQL string", async () => {
    let capturedQuery = "";
    (invoke as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") {
        capturedQuery = args?.query ?? "";
        return Promise.resolve({ columns: [], rows: [], truncated: false });
      }
      return Promise.resolve(null);
    });
    render(<AuditTrail org="o'brien" />);
    await waitFor(() => {
      expect(capturedQuery).toContain("o''brien");
      // The query must still be well-formed: the WHERE clause's string literal is properly
      // closed (no dangling unescaped quote that would break out of it).
      expect(capturedQuery).toContain("org_id = 'o''brien'");
    });
  });

  it("handles invoke error gracefully", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "list_saved_views") return Promise.resolve([]);
      if (cmd === "run_sql") return Promise.reject(new Error("db error"));
      return Promise.resolve(null);
    });
    render(<AuditTrail org="acme" />);
    await waitFor(() => {
      expect(screen.getByText(/db error/i)).toBeDefined();
    });
  });
});
