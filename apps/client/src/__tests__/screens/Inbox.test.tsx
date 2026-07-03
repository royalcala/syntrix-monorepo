import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Inbox } from "../../screens/Inbox";

describe("Inbox", () => {
  function Wrapped({ children }: { children: React.ReactNode }) {
    return <MemoryRouter>{children}</MemoryRouter>;
  }

  it("shows empty state when no invites", () => {
    render(<Wrapped><Inbox invites={[]} onAccept={() => {}} /></Wrapped>);
    expect(screen.getByText(/No notifications yet/)).toBeDefined();
  });

  it("renders invite list", () => {
    const invites = [
      { org_name: "acme", role: "sales", tickets: [{ ns: "t1", ticket: "ticket-1" }] },
      { org_name: "beta", role: "admin", tickets: [{ ns: "t2", ticket: "ticket-2" }] },
    ];
    render(<Wrapped><Inbox invites={invites} onAccept={() => {}} /></Wrapped>);
    expect(screen.getByText("acme")).toBeDefined();
    expect(screen.getByText("beta")).toBeDefined();
  });

  it("calls onAccept when accept button clicked", () => {
    const onAccept = vi.fn();
    const invites = [
      { org_name: "test-org", role: "viewer", tickets: [{ ns: "ns1", ticket: "ticket-1" }] },
    ];
    render(<Wrapped><Inbox invites={invites} onAccept={onAccept} /></Wrapped>);
    const acceptBtn = screen.getByRole("button", { name: /accept/i });
    fireEvent.click(acceptBtn);
    expect(onAccept).toHaveBeenCalledWith(invites[0]);
  });

  it("shows badge with invite count", () => {
    const invites = [
      { org_name: "a", role: "r1", tickets: [{ ns: "n1", ticket: "t1" }] },
      { org_name: "b", role: "r2", tickets: [{ ns: "n2", ticket: "t2" }] },
    ];
    render(<Wrapped><Inbox invites={invites} onAccept={() => {}} /></Wrapped>);
    expect(screen.getByText(/2 new/)).toBeDefined();
  });
});
