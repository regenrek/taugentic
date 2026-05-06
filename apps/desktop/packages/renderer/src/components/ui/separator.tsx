import * as React from "react";
import { Separator as BaseSeparator } from "@base-ui-components/react/separator";

import { cn } from "@/lib/ui/cn";

export type SeparatorProps = React.ComponentPropsWithoutRef<typeof BaseSeparator>;

export const Separator = React.forwardRef<HTMLDivElement, SeparatorProps>(function Separator(
  { className, orientation = "horizontal", ...props },
  ref,
) {
  return (
    <BaseSeparator
      ref={ref}
      orientation={orientation}
      className={cn(
        "shrink-0 bg-[var(--border)]",
        orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
        className,
      )}
      {...props}
    />
  );
});
