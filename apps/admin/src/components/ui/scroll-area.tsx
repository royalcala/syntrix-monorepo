import * as React from "react";

function ScrollArea({ className, children, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={`overflow-auto ${className ?? ""}`} {...props}>
      {children}
    </div>
  );
}

export { ScrollArea };
