import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/ui/cn";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 border border-[var(--border)] rounded-[var(--radius)] text-sm font-medium uppercase tracking-[0.08em] font-[var(--font-mono)] transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))] disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-[var(--fg)] text-[var(--bg)] border-[var(--fg)] hover:bg-[var(--fg)]/90",
        secondary:
          "bg-[var(--bg-subtle,transparent)] text-[var(--fg)] hover:bg-[var(--bg-hover,var(--border))]",
        outline: "bg-transparent text-[var(--fg)] hover:bg-[var(--bg-hover,var(--border))]",
        destructive:
          "bg-[var(--danger,var(--fg))] text-[var(--bg)] border-[var(--danger,var(--fg))] hover:opacity-90",
        ghost:
          "border-transparent bg-transparent text-[var(--fg)] hover:bg-[var(--bg-hover,var(--border))]",
      },
      size: {
        sm: "h-8 px-3 text-xs",
        default: "h-9 px-4",
        lg: "h-10 px-5 text-sm",
        icon: "h-9 w-9 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  };

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { asChild = false, className, size, variant, children, ...props },
  ref,
) {
  const resolvedVariant = variant ?? "default";
  const resolvedSize = size ?? "default";
  const mergedClassName = cn(
    buttonVariants({ size: resolvedSize, variant: resolvedVariant }),
    className,
  );

  if (asChild && React.isValidElement(children)) {
    const child = children as React.ReactElement<Record<string, unknown>>;
    const childProps = child.props as Record<string, unknown>;
    return React.cloneElement(child, {
      ...(props as Record<string, unknown>),
      ...childProps,
      className: cn(mergedClassName, childProps.className as string | undefined),
      "data-variant": resolvedVariant,
      "data-size": resolvedSize,
      ref,
    } as Record<string, unknown>);
  }

  return (
    <button
      ref={ref}
      className={mergedClassName}
      data-variant={resolvedVariant}
      data-size={resolvedSize}
      {...props}
    >
      {children}
    </button>
  );
});

export { buttonVariants };
