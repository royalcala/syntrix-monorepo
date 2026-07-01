import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CreateOrg } from "../../screens/CreateOrg";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("CreateOrg", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders create org form", () => {
    render(<CreateOrg nodeId="test-node" onCreated={() => {}} />);
    expect(screen.getByText("Crear organización")).toBeDefined();
  });

  it("calls create_org on submit and calls onCreated", async () => {
    const onCreated = vi.fn();
    (invoke as any).mockResolvedValue(undefined);
    render(<CreateOrg nodeId="test-node" onCreated={onCreated} />);
    const input = screen.getByPlaceholderText("Nombre de la organización");
    fireEvent.change(input, { target: { value: "TestOrg" } });
    fireEvent.click(screen.getByText("Crear"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_org", { name: "TestOrg" });
      expect(onCreated).toHaveBeenCalled();
    });
  });

  it("shows error when create_org fails", async () => {
    const onCreated = vi.fn();
    (invoke as any).mockRejectedValue(new Error("org exists"));
    render(<CreateOrg nodeId="test-node" onCreated={onCreated} />);
    fireEvent.change(screen.getByPlaceholderText("Nombre de la organización"), { target: { value: "Dup" } });
    fireEvent.click(screen.getByText("Crear"));
    await waitFor(() => {
      expect(screen.getByText(/org exists/)).toBeDefined();
      expect(onCreated).not.toHaveBeenCalled();
    });
  });

  it("does not call invoke with empty name", async () => {
    render(<CreateOrg nodeId="test-node" onCreated={() => {}} />);
    fireEvent.click(screen.getByText("Crear"));
    expect(invoke).not.toHaveBeenCalled();
  });
});
