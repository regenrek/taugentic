import * as React from "react";
import { ContextMenu as BaseContextMenu } from "@base-ui-components/react/context-menu";

import { cn } from "@/lib/ui/cn";

const Root = BaseContextMenu.Root;
const Trigger = BaseContextMenu.Trigger;
const Portal = BaseContextMenu.Portal;
const Group = BaseContextMenu.Group;
const RadioGroup = BaseContextMenu.RadioGroup;
const SubMenu = BaseContextMenu.SubmenuRoot;
const Positioner = BaseContextMenu.Positioner;

export type ContextMenuContentProps = React.ComponentPropsWithoutRef<
  typeof BaseContextMenu.Popup
> & {
  positionerClassName?: string;
};

const Content = React.forwardRef<HTMLDivElement, ContextMenuContentProps>(
  function ContextMenuContent({ className, positionerClassName, children, ...props }, ref) {
    return (
      <BaseContextMenu.Portal>
        <BaseContextMenu.Positioner className={cn("z-50 outline-none", positionerClassName)}>
          <BaseContextMenu.Popup
            ref={ref}
            className={cn(
              "min-w-[10rem] overflow-hidden rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-raised,var(--bg))] p-1 text-sm text-[var(--fg)] shadow-md outline-none",
              "data-[ending-style]:opacity-0 data-[starting-style]:opacity-0 transition-opacity",
              className,
            )}
            {...props}
          >
            {children}
          </BaseContextMenu.Popup>
        </BaseContextMenu.Positioner>
      </BaseContextMenu.Portal>
    );
  },
);

export type ContextMenuItemProps = React.ComponentPropsWithoutRef<typeof BaseContextMenu.Item> & {
  inset?: boolean;
  destructive?: boolean;
};

const Item = React.forwardRef<HTMLDivElement, ContextMenuItemProps>(function ContextMenuItem(
  { className, inset, destructive, ...props },
  ref,
) {
  return (
    <BaseContextMenu.Item
      ref={ref}
      data-destructive={destructive ? "" : undefined}
      className={cn(
        "relative flex cursor-default select-none items-center gap-2 rounded-[var(--radius-sm,var(--radius))] px-2 py-1.5 text-sm outline-none transition-colors",
        "data-[highlighted]:bg-[var(--surface-overlay,var(--bg-sunken))] data-[highlighted]:text-[var(--fg)]",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        inset && "pl-8",
        destructive && "text-[var(--status-failed)] data-[highlighted]:text-[var(--status-failed)]",
        className,
      )}
      {...props}
    />
  );
});

const Label = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseContextMenu.GroupLabel> & { inset?: boolean }
>(function ContextMenuLabel({ className, inset, ...props }, ref) {
  return (
    <BaseContextMenu.GroupLabel
      ref={ref}
      className={cn(
        "px-2 py-1.5 text-[10px] font-medium uppercase tracking-[0.18em] font-[var(--font-mono)] text-[var(--fg-mute,var(--fg-dim))]",
        inset && "pl-8",
        className,
      )}
      {...props}
    />
  );
});

const Separator = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseContextMenu.Separator>
>(function ContextMenuSeparator({ className, ...props }, ref) {
  return (
    <BaseContextMenu.Separator
      ref={ref}
      className={cn("-mx-1 my-1 h-px bg-[var(--border)]", className)}
      {...props}
    />
  );
});

const SubTrigger = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseContextMenu.SubmenuTrigger> & { inset?: boolean }
>(function ContextMenuSubTrigger({ className, inset, children, ...props }, ref) {
  return (
    <BaseContextMenu.SubmenuTrigger
      ref={ref}
      className={cn(
        "relative flex cursor-default select-none items-center gap-2 rounded-[var(--radius-sm,var(--radius))] px-2 py-1.5 text-sm outline-none",
        "data-[highlighted]:bg-[var(--surface-overlay,var(--bg-sunken))] data-[highlighted]:text-[var(--fg)]",
        "data-[popup-open]:bg-[var(--surface-overlay,var(--bg-sunken))]",
        inset && "pl-8",
        className,
      )}
      {...props}
    >
      {children}
    </BaseContextMenu.SubmenuTrigger>
  );
});

const SubContent = React.forwardRef<HTMLDivElement, ContextMenuContentProps>(
  function ContextMenuSubContent({ className, positionerClassName, children, ...props }, ref) {
    return (
      <BaseContextMenu.Portal>
        <BaseContextMenu.Positioner className={cn("z-50 outline-none", positionerClassName)}>
          <BaseContextMenu.Popup
            ref={ref}
            className={cn(
              "min-w-[8rem] overflow-hidden rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-raised,var(--bg))] p-1 text-sm text-[var(--fg)] shadow-md outline-none",
              className,
            )}
            {...props}
          >
            {children}
          </BaseContextMenu.Popup>
        </BaseContextMenu.Positioner>
      </BaseContextMenu.Portal>
    );
  },
);

const CheckboxItem = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseContextMenu.CheckboxItem>
>(function ContextMenuCheckboxItem({ className, children, ...props }, ref) {
  return (
    <BaseContextMenu.CheckboxItem
      ref={ref}
      className={cn(
        "relative flex cursor-default select-none items-center gap-2 rounded-[var(--radius-sm,var(--radius))] py-1.5 pl-8 pr-2 text-sm outline-none",
        "data-[highlighted]:bg-[var(--surface-overlay,var(--bg-sunken))] data-[highlighted]:text-[var(--fg)]",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    >
      <span className="absolute left-2 flex h-4 w-4 items-center justify-center">
        <BaseContextMenu.CheckboxItemIndicator className="text-[var(--fg)]">
          <svg viewBox="0 0 16 16" className="h-3 w-3" aria-hidden="true">
            <path
              d="M3 8.5l3 3 7-7"
              stroke="currentColor"
              strokeWidth="2"
              fill="none"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </BaseContextMenu.CheckboxItemIndicator>
      </span>
      {children}
    </BaseContextMenu.CheckboxItem>
  );
});

const RadioItem = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseContextMenu.RadioItem>
>(function ContextMenuRadioItem({ className, children, ...props }, ref) {
  return (
    <BaseContextMenu.RadioItem
      ref={ref}
      className={cn(
        "relative flex cursor-default select-none items-center gap-2 rounded-[var(--radius-sm,var(--radius))] py-1.5 pl-8 pr-2 text-sm outline-none",
        "data-[highlighted]:bg-[var(--surface-overlay,var(--bg-sunken))] data-[highlighted]:text-[var(--fg)]",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    >
      <span className="absolute left-2 flex h-4 w-4 items-center justify-center">
        <BaseContextMenu.RadioItemIndicator className="text-[var(--fg)]">
          <span className="block h-1.5 w-1.5 rounded-full bg-current" />
        </BaseContextMenu.RadioItemIndicator>
      </span>
      {children}
    </BaseContextMenu.RadioItem>
  );
});

export const ContextMenu = Object.assign(Root, {
  Root,
  Trigger,
  Portal,
  Positioner,
  Content,
  Item,
  Label,
  Group,
  Separator,
  SubMenu,
  SubTrigger,
  SubContent,
  CheckboxItem,
  RadioGroup,
  RadioItem,
});
