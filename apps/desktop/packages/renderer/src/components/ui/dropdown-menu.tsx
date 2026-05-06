import * as React from "react";
import { Menu as BaseMenu } from "@base-ui-components/react/menu";

import { cn } from "@/lib/ui/cn";

const Root = BaseMenu.Root;
const Trigger = BaseMenu.Trigger;
const Portal = BaseMenu.Portal;
const Group = BaseMenu.Group;
const RadioGroup = BaseMenu.RadioGroup;
const SubMenu = BaseMenu.SubmenuRoot;
const Positioner = BaseMenu.Positioner;

export type DropdownMenuContentProps = React.ComponentPropsWithoutRef<typeof BaseMenu.Popup> & {
  sideOffset?: number;
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  positionerClassName?: string;
};

const Content = React.forwardRef<HTMLDivElement, DropdownMenuContentProps>(
  function DropdownMenuContent(
    {
      className,
      sideOffset = 6,
      align = "start",
      side = "bottom",
      positionerClassName,
      children,
      ...props
    },
    ref,
  ) {
    return (
      <BaseMenu.Portal>
        <BaseMenu.Positioner
          sideOffset={sideOffset}
          align={align}
          side={side}
          className={cn("z-50 outline-none", positionerClassName)}
        >
          <BaseMenu.Popup
            ref={ref}
            className={cn(
              "min-w-[10rem] overflow-hidden rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-raised,var(--bg))] p-1 text-sm text-[var(--fg)] shadow-md outline-none",
              "data-[ending-style]:opacity-0 data-[starting-style]:opacity-0 transition-opacity",
              className,
            )}
            {...props}
          >
            {children}
          </BaseMenu.Popup>
        </BaseMenu.Positioner>
      </BaseMenu.Portal>
    );
  },
);

export type DropdownMenuItemProps = React.ComponentPropsWithoutRef<typeof BaseMenu.Item> & {
  inset?: boolean;
  destructive?: boolean;
};

const Item = React.forwardRef<HTMLDivElement, DropdownMenuItemProps>(function DropdownMenuItem(
  { className, inset, destructive, ...props },
  ref,
) {
  return (
    <BaseMenu.Item
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
  React.ComponentPropsWithoutRef<typeof BaseMenu.GroupLabel> & { inset?: boolean }
>(function DropdownMenuLabel({ className, inset, ...props }, ref) {
  return (
    <BaseMenu.GroupLabel
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
  React.ComponentPropsWithoutRef<typeof BaseMenu.Separator>
>(function DropdownMenuSeparator({ className, ...props }, ref) {
  return (
    <BaseMenu.Separator
      ref={ref}
      className={cn("-mx-1 my-1 h-px bg-[var(--border)]", className)}
      {...props}
    />
  );
});

const SubTrigger = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseMenu.SubmenuTrigger> & { inset?: boolean }
>(function DropdownMenuSubTrigger({ className, inset, children, ...props }, ref) {
  return (
    <BaseMenu.SubmenuTrigger
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
    </BaseMenu.SubmenuTrigger>
  );
});

const SubContent = React.forwardRef<HTMLDivElement, DropdownMenuContentProps>(
  function DropdownMenuSubContent(
    {
      className,
      sideOffset = 4,
      align = "start",
      side = "right",
      positionerClassName,
      children,
      ...props
    },
    ref,
  ) {
    return (
      <BaseMenu.Portal>
        <BaseMenu.Positioner
          sideOffset={sideOffset}
          align={align}
          side={side}
          className={cn("z-50 outline-none", positionerClassName)}
        >
          <BaseMenu.Popup
            ref={ref}
            className={cn(
              "min-w-[8rem] overflow-hidden rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-raised,var(--bg))] p-1 text-sm text-[var(--fg)] shadow-md outline-none",
              className,
            )}
            {...props}
          >
            {children}
          </BaseMenu.Popup>
        </BaseMenu.Positioner>
      </BaseMenu.Portal>
    );
  },
);

const CheckboxItem = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseMenu.CheckboxItem>
>(function DropdownMenuCheckboxItem({ className, children, ...props }, ref) {
  return (
    <BaseMenu.CheckboxItem
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
        <BaseMenu.CheckboxItemIndicator className="text-[var(--fg)]">
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
        </BaseMenu.CheckboxItemIndicator>
      </span>
      {children}
    </BaseMenu.CheckboxItem>
  );
});

const RadioItem = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseMenu.RadioItem>
>(function DropdownMenuRadioItem({ className, children, ...props }, ref) {
  return (
    <BaseMenu.RadioItem
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
        <BaseMenu.RadioItemIndicator className="text-[var(--fg)]">
          <span className="block h-1.5 w-1.5 rounded-full bg-current" />
        </BaseMenu.RadioItemIndicator>
      </span>
      {children}
    </BaseMenu.RadioItem>
  );
});

export const DropdownMenu = Object.assign(Root, {
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
