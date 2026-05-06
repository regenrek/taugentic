import * as React from "react";
import { Tooltip as BaseTooltip } from "@base-ui-components/react/tooltip";

import { cn } from "@/lib/ui/cn";

const Provider = BaseTooltip.Provider;

const Root = BaseTooltip.Root;

const Trigger = BaseTooltip.Trigger;

const Portal = BaseTooltip.Portal;

export type TooltipContentProps = React.ComponentPropsWithoutRef<typeof BaseTooltip.Popup> & {
  sideOffset?: number;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  positionerClassName?: string;
};

const Content = React.forwardRef<HTMLDivElement, TooltipContentProps>(function TooltipContent(
  {
    className,
    sideOffset = 6,
    side = "top",
    align = "center",
    positionerClassName,
    children,
    ...props
  },
  ref,
) {
  return (
    <BaseTooltip.Portal>
      <BaseTooltip.Positioner
        sideOffset={sideOffset}
        side={side}
        align={align}
        className={cn("z-50 outline-none", positionerClassName)}
      >
        <BaseTooltip.Popup
          ref={ref}
          className={cn(
            "rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-elevated,var(--bg))] px-2 py-1 text-[11px] uppercase tracking-[0.12em] font-[var(--font-mono)] text-[var(--fg)] outline-none",
            className,
          )}
          {...props}
        >
          {children}
        </BaseTooltip.Popup>
      </BaseTooltip.Positioner>
    </BaseTooltip.Portal>
  );
});

export const Tooltip = Object.assign(Root, {
  Provider,
  Root,
  Trigger,
  Portal,
  Content,
});
