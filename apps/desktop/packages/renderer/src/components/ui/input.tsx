import * as React from "react";

import { cn } from "@/lib/ui/cn";

export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(function Input({ className, type = "text", ...props }, ref) {
  return (
    <input
      ref={ref}
      type={type}
      className={cn(
        "flex h-9 w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 text-sm text-[var(--fg)] transition-colors placeholder:text-[var(--fg-muted,var(--fg))]/50 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
});
