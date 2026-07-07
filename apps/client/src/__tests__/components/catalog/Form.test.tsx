import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { FormRenderer } from "@syntrix/ui/components/catalog/FormComponent";
import type { FormComponent } from "@syntrix/ui/components/catalog/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}), emit: vi.fn() }));

const formDef: FormComponent = {
  id: "f1",
  type: "form",
  entity: "customers",
  action: "create",
  fields: [
    { key: "name", label: "Name", type: "text", required: true },
    { key: "email", label: "Email", type: "text" },
  ],
  validations: [
    { field: "name", op: "required", value: true, message: "Name is required" },
  ],
  submitLabel: "Save",
};

describe("FormRenderer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders form fields and submit button", () => {
    render(
      <MemoryRouter>
        <FormRenderer component={formDef} onSubmit={async () => {}} />
      </MemoryRouter>
    );
    expect(screen.getByText("Name")).toBeDefined();
    expect(screen.getByText("Email")).toBeDefined();
    expect(screen.getByText("Save")).toBeDefined();
  });

  it("validates required fields on submit", async () => {
    render(
      <MemoryRouter>
        <FormRenderer component={formDef} onSubmit={async () => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("Save"));
    await waitFor(() => {
      expect(screen.getByText("Name is required")).toBeDefined();
    });
  });

  it("calls onSubmit with form data when valid", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <MemoryRouter>
        <FormRenderer component={formDef} onSubmit={onSubmit} />
      </MemoryRouter>
    );
    // label is not associated via htmlFor, find all textboxes and use the first one
    const inputs = screen.getAllByRole("textbox");
    fireEvent.change(inputs[0], { target: { value: "Alice" } });
    fireEvent.click(screen.getByText("Save"));
    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: "Alice" }));
    });
  });

  it("initializes with initialData when provided", () => {
    render(
      <MemoryRouter>
        <FormRenderer component={formDef} initialData={{ name: "Bob", email: "bob@test.com" }} onSubmit={async () => {}} />
      </MemoryRouter>
    );
    const inputs = screen.getAllByRole("textbox");
    expect((inputs[0] as HTMLInputElement).value).toBe("Bob");
  });
});
