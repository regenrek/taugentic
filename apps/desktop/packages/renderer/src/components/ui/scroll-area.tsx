import * as React from "react";
import { ScrollArea as BaseScrollArea } from "@base-ui-components/react/scroll-area";

import { cn } from "@/lib/ui/cn";

type RootElementProps = React.ComponentPropsWithoutRef<typeof BaseScrollArea.Root>;
type ViewportElementProps = React.ComponentPropsWithoutRef<typeof BaseScrollArea.Viewport>;
type ScrollbarElementProps = React.ComponentPropsWithoutRef<typeof BaseScrollArea.Scrollbar>;
type ThumbElementProps = React.ComponentPropsWithoutRef<typeof BaseScrollArea.Thumb>;
type CornerElementProps = React.ComponentPropsWithoutRef<typeof BaseScrollArea.Corner>;

const Root = React.forwardRef<HTMLDivElement, RootElementProps>(function ScrollAreaRoot(
  { className, ...props },
  ref,
) {
  return (
    <BaseScrollArea.Root
      ref={ref}
      className={cn("relative overflow-hidden", className)}
      {...props}
    />
  );
});

const Viewport = React.forwardRef<HTMLDivElement, ViewportElementProps>(function ScrollAreaViewport(
  { className, ...props },
  ref,
) {
  return (
    <BaseScrollArea.Viewport
      ref={ref}
      className={cn("size-full overscroll-contain", className)}
      {...props}
    />
  );
});

const Scrollbar = React.forwardRef<HTMLDivElement, ScrollbarElementProps>(
  function ScrollAreaScrollbar({ className, orientation = "vertical", ...props }, ref) {
    return (
      <BaseScrollArea.Scrollbar
        ref={ref}
        orientation={orientation}
        className={cn(
          "flex touch-none select-none bg-transparent p-px transition-opacity",
          orientation === "vertical" ? "h-full w-1.5" : "h-1.5 w-full",
          className,
        )}
        {...props}
      />
    );
  },
);

const Thumb = React.forwardRef<HTMLDivElement, ThumbElementProps>(function ScrollAreaThumb(
  { className, ...props },
  ref,
) {
  return (
    <BaseScrollArea.Thumb
      ref={ref}
      className={cn(
        "relative flex-1 rounded-[var(--radius)] bg-[var(--border)] hover:bg-[var(--fg-muted,var(--fg))]/40",
        className,
      )}
      {...props}
    />
  );
});

const Corner = React.forwardRef<HTMLDivElement, CornerElementProps>(function ScrollAreaCorner(
  { className, ...props },
  ref,
) {
  return <BaseScrollArea.Corner ref={ref} className={cn("bg-transparent", className)} {...props} />;
});

export type ScrollAreaProps = RootElementProps & {
  viewportClassName?: string;
  scrollbarClassName?: string;
  children?: React.ReactNode;
};

const ScrollAreaBase = React.forwardRef<HTMLDivElement, ScrollAreaProps>(function ScrollArea(
  { className, viewportClassName, scrollbarClassName, children, ...props },
  ref,
) {
  return (
    <Root ref={ref} className={className} {...props}>
      <Viewport className={viewportClassName}>{children}</Viewport>
      <Scrollbar className={scrollbarClassName}>
        <Thumb />
      </Scrollbar>
      <Corner />
    </Root>
  );
});

export const ScrollArea = Object.assign(ScrollAreaBase, {
  Root,
  Viewport,
  Scrollbar,
  Thumb,
  Corner,
});
