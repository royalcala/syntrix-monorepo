import { useEffect, ReactNode } from "react";

interface SheetProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

export function Sheet({ open, onClose, children }: SheetProps) {
  useEffect(() => {
    if (open) document.body.style.overflow = "hidden";
    else document.body.style.overflow = "";
    return () => { document.body.style.overflow = ""; };
  }, [open]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 lg:hidden">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="fixed left-0 top-0 bottom-0 w-72 bg-background border-r border-border shadow-xl">
        {children}
      </div>
    </div>
  );
}

interface SheetContentProps {
  className?: string;
  children: ReactNode;
  side?: "left" | "right";
}

export function SheetContent({ className = "", children, side = "left" }: SheetContentProps) {
  return (
    <div className={`fixed ${side}-0 top-0 bottom-0 w-72 bg-background border-${side === "left" ? "r" : "l"} border-border shadow-xl ${className}`}>
      {children}
    </div>
  );
}
