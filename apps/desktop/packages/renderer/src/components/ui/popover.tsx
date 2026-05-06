import * as React from "react";
import { Popover as BasePopover } from "@base-ui-components/react/popover";

import { cn } from "@/lib/ui/cn";

const Root = BasePopover.Root;

const Trigger = BasePopover.Trigger;

const Close = BasePopover.Close;

const Portal = BasePopover.Portal;

export type PopoverContentProps = React.ComponentPropsWithoutRef<typeof BasePopover.Popup> & {
  sideOffset?: number;
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  positionerClassName?: string;
};

const Content = React.forwardRef<HTMLDivElement, PopoverContentProps>(function PopoverContent(
  {
    className,
    sideOffset = 8,
    align = "center",
    side = "bottom",
    positionerClassName,
    children,
    ...props
  },
  ref,
) {
  return (
    <BasePopover.Portal>
      <BasePopover.Positioner
        sideOffset={sideOffset}
        align={align}
        side={side}
        className={cn("z-50 outline-none", positionerClassName)}
      >
        <BasePopover.Popup
          ref={ref}
          className={cn(
            "min-w-[10rem] rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-elevated,var(--bg))] p-3 text-sm text-[var(--fg)] outline-none",
            className,
          )}
          {...props}
        >
          {children}
        </BasePopover.Popup>
      </BasePopover.Positioner>
    </BasePopover.Portal>
  );
});

const Arrow = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BasePopover.Arrow>
>(function PopoverArrow({ className, ...props }, ref) {
  return (
    <BasePopover.Arrow
      ref={ref}
      className={cn("fill-[var(--bg-elevated,var(--bg))] stroke-[var(--border)]", className)}
      {...props}
    />
  );
});

export const Popover = Object.assign(Root, {
  Root,
  Trigger,
  Close,
  Portal,
  Content,
  Arrow,
});
