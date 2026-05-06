import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/ui/cn";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-[var(--radius)] border border-[var(--border)] px-2 py-[2px] text-[10px] uppercase tracking-[0.18em] font-medium font-[var(--font-mono)]",
  {
    variants: {
      variant: {
        default: "bg-[var(--fg)] text-[var(--bg)] border-[var(--fg)]",
        secondary: "bg-[var(--bg-subtle,transparent)] text-[var(--fg)]",
        outline: "bg-transparent text-[var(--fg-muted,var(--fg))]",
        destructive:
          "bg-transparent text-[var(--danger,var(--fg))] border-[var(--danger,var(--border))]",
        accent:
          "bg-transparent text-[var(--accent-fg,var(--fg))] border-[var(--accent,var(--border))]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export type BadgeProps = React.HTMLAttributes<HTMLDivElement> & VariantProps<typeof badgeVariants>;

export const Badge = React.forwardRef<HTMLDivElement, BadgeProps>(function Badge(
  { className, variant, ...props },
  ref,
) {
  const resolvedVariant = variant ?? "default";
  return (
    <div
      ref={ref}
      className={cn(badgeVariants({ variant: resolvedVariant }), className)}
      data-variant={resolvedVariant}
      {...props}
    />
  );
});

export { badgeVariants };
