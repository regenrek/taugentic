import * as React from "react";

import { cn } from "@/lib/ui/cn";

export type StatusTone = "active" | "waiting" | "failed" | "completed" | "cancelled" | "idle";

export interface StatusDotProps extends Omit<React.HTMLAttributes<HTMLSpanElement>, "children"> {
  tone: StatusTone;
  label?: string;
}

export const StatusDot = React.forwardRef<HTMLSpanElement, StatusDotProps>(function StatusDot(
  { tone, label, className, style, ...props },
  ref,
) {
  const dotStyle: React.CSSProperties = {
    backgroundColor: `var(--status-${tone})`,
    ...style,
  };

  if (label === undefined) {
    return (
      <span
        ref={ref}
        aria-hidden="true"
        data-tone={tone}
        data-status-dot=""
        className={cn("inline-block h-[6px] w-[6px] rounded-full align-middle", className)}
        style={dotStyle}
        {...props}
      />
    );
  }

  return (
    <span
      ref={ref}
      data-tone={tone}
      className={cn(
        "inline-flex items-center gap-2 text-[11px] uppercase tracking-[0.14em] font-[var(--font-mono)] text-[var(--fg)]",
        className,
      )}
      {...props}
    >
      <span
        aria-hidden="true"
        data-status-dot=""
        className="inline-block h-[6px] w-[6px] rounded-full"
        style={dotStyle}
      />
      <span>{label}</span>
    </span>
  );
});
