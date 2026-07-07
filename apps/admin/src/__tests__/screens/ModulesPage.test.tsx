import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ModulesPage } from "../../screens/ModulesPage";

describe("ModulesPage", () => {
  it("renders module templates", () => {
    render(
      <MemoryRouter>
        <ModulesPage />
      </MemoryRouter>
    );
    expect(screen.getByText(/ERP Básico/i)).toBeDefined();
    expect(screen.getByText(/Familiar/i)).toBeDefined();
    expect(screen.getByText(/Comunidad/i)).toBeDefined();
  });

  it("shows coming phase label for each template", () => {
    render(
      <MemoryRouter>
        <ModulesPage />
      </MemoryRouter>
    );
    const comingLabels = screen.getAllByText(/Fase B/i);
    expect(comingLabels.length).toBeGreaterThanOrEqual(3);
  });
});
