import * as React from "react";
import { Collapsible as BaseCollapsible } from "@base-ui-components/react/collapsible";

import { cn } from "@/lib/ui/cn";

const Root = BaseCollapsible.Root;

const Trigger = React.forwardRef<
  HTMLButtonElement,
  React.ComponentPropsWithoutRef<typeof BaseCollapsible.Trigger>
>(function CollapsibleTrigger({ className, ...props }, ref) {
  return (
    <BaseCollapsible.Trigger
      ref={ref}
      className={cn(
        "inline-flex items-center gap-2 border border-transparent text-sm text-[var(--fg)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))]",
        className,
      )}
      {...props}
    />
  );
});

const Content = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseCollapsible.Panel>
>(function CollapsibleContent({ className, ...props }, ref) {
  return (
    <BaseCollapsible.Panel
      ref={ref}
      className={cn(
        "overflow-hidden text-sm text-[var(--fg)] transition-[height] data-[ending-style]:h-0 data-[starting-style]:h-0",
        className,
      )}
      {...props}
    />
  );
});

export const Collapsible = Object.assign(Root, {
  Root,
  Trigger,
  Content,
});
