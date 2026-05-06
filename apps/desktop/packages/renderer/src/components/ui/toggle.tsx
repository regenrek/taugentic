import * as React from "react";
import { Toggle as BaseToggle } from "@base-ui-components/react/toggle";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/ui/cn";

const toggleVariants = cva(
  "inline-flex items-center justify-center gap-2 rounded-[var(--radius)] text-sm font-medium uppercase tracking-[0.08em] font-[var(--font-mono)] transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))] disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default:
          "border border-transparent bg-transparent text-[var(--fg-dim,var(--fg))] hover:bg-[var(--surface-overlay,var(--bg-sunken))] hover:text-[var(--fg)] data-[pressed]:bg-[var(--accent)] data-[pressed]:text-[var(--accent-fg)]",
        outline:
          "border border-[var(--border)] bg-transparent text-[var(--fg)] hover:bg-[var(--surface-overlay,var(--bg-sunken))] data-[pressed]:bg-[var(--accent)] data-[pressed]:text-[var(--accent-fg)] data-[pressed]:border-[var(--accent)]",
        ghost:
          "border border-transparent bg-transparent text-[var(--fg-dim,var(--fg))] hover:bg-[var(--surface-overlay,var(--bg-sunken))] hover:text-[var(--fg)] data-[pressed]:bg-[var(--surface-overlay,var(--bg-sunken))] data-[pressed]:text-[var(--fg)]",
      },
      size: {
        sm: "h-7 px-2 text-xs",
        md: "h-9 px-3",
        icon: "h-9 w-9 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
);

export type ToggleProps = React.ComponentPropsWithoutRef<typeof BaseToggle> &
  VariantProps<typeof toggleVariants>;

export const Toggle = React.forwardRef<HTMLButtonElement, ToggleProps>(function Toggle(
  { className, variant, size, ...props },
  ref,
) {
  const resolvedVariant = variant ?? "default";
  const resolvedSize = size ?? "md";
  return (
    <BaseToggle
      ref={ref}
      data-variant={resolvedVariant}
      data-size={resolvedSize}
      className={cn(toggleVariants({ variant: resolvedVariant, size: resolvedSize }), className)}
      {...props}
    />
  );
});

export { toggleVariants };
