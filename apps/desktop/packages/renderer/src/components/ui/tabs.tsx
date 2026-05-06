import * as React from "react";
import { Tabs as BaseTabs } from "@base-ui-components/react/tabs";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/ui/cn";

const Root = React.forwardRef<HTMLDivElement, React.ComponentPropsWithoutRef<typeof BaseTabs.Root>>(
  function TabsRoot({ className, ...props }, ref) {
    return <BaseTabs.Root ref={ref} className={cn("flex flex-col gap-2", className)} {...props} />;
  },
);

const tabsListVariants = cva(
  "relative inline-flex items-center gap-1 font-[var(--font-mono)] text-xs uppercase tracking-[0.08em]",
  {
    variants: {
      variant: {
        default:
          "rounded-[var(--radius)] border border-[var(--border)] bg-[var(--tabs-list-bg,var(--bg-raised))] p-1",
        line: "border-b border-[var(--border)] bg-transparent p-0",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export type TabsListProps = React.ComponentPropsWithoutRef<typeof BaseTabs.List> &
  VariantProps<typeof tabsListVariants>;

const List = React.forwardRef<HTMLDivElement, TabsListProps>(function TabsList(
  { className, variant, ...props },
  ref,
) {
  const resolvedVariant = variant ?? "default";
  return (
    <BaseTabs.List
      ref={ref}
      data-variant={resolvedVariant}
      className={cn(tabsListVariants({ variant: resolvedVariant }), className)}
      {...props}
    />
  );
});

const tabsTriggerVariants = cva(
  "relative inline-flex h-7 cursor-pointer select-none items-center justify-center whitespace-nowrap px-3 text-[var(--tabs-trigger-inactive-fg,var(--fg-dim))] transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))] disabled:pointer-events-none disabled:opacity-50 data-[selected]:text-[var(--tabs-trigger-active-fg,var(--fg))]",
  {
    variants: {
      variant: {
        default:
          "rounded-[var(--radius-sm,var(--radius))] hover:text-[var(--fg)] data-[selected]:bg-[var(--surface-overlay,var(--bg))]",
        line: "rounded-none hover:text-[var(--fg)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export type TabsTriggerProps = React.ComponentPropsWithoutRef<typeof BaseTabs.Tab> &
  VariantProps<typeof tabsTriggerVariants>;

const Trigger = React.forwardRef<HTMLButtonElement, TabsTriggerProps>(function TabsTrigger(
  { className, variant, ...props },
  ref,
) {
  const resolvedVariant = variant ?? "default";
  return (
    <BaseTabs.Tab
      ref={ref}
      data-variant={resolvedVariant}
      className={cn(tabsTriggerVariants({ variant: resolvedVariant }), className)}
      {...props}
    />
  );
});

const Content = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseTabs.Panel>
>(function TabsContent({ className, ...props }, ref) {
  return (
    <BaseTabs.Panel
      ref={ref}
      className={cn(
        "outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))]",
        className,
      )}
      {...props}
    />
  );
});

const Indicator = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseTabs.Indicator>
>(function TabsIndicator({ className, ...props }, ref) {
  return (
    <BaseTabs.Indicator
      ref={ref}
      className={cn(
        "absolute left-[var(--active-tab-left)] top-[var(--active-tab-top)] h-[var(--active-tab-height)] w-[var(--active-tab-width)] bg-[var(--tabs-indicator,var(--fg))] transition-all duration-150",
        className,
      )}
      {...props}
    />
  );
});

export const Tabs = Object.assign(Root, {
  Root,
  List,
  Trigger,
  Content,
  Indicator,
});

export { tabsListVariants, tabsTriggerVariants };
