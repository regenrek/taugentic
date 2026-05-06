import * as React from "react";
import { Dialog as BaseDialog } from "@base-ui-components/react/dialog";

import { cn } from "@/lib/ui/cn";

const Root = BaseDialog.Root;

const Trigger = BaseDialog.Trigger;

const Portal = BaseDialog.Portal;

const Close = BaseDialog.Close;

const Overlay = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<typeof BaseDialog.Backdrop>
>(function DialogOverlay({ className, ...props }, ref) {
  return (
    <BaseDialog.Backdrop
      ref={ref}
      className={cn(
        "fixed inset-0 z-50 bg-[var(--bg)]/80 data-[ending-style]:opacity-0 data-[starting-style]:opacity-0 transition-opacity",
        className,
      )}
      {...props}
    />
  );
});

export type DialogContentProps = React.ComponentPropsWithoutRef<typeof BaseDialog.Popup> & {
  overlayClassName?: string;
};

const Content = React.forwardRef<HTMLDivElement, DialogContentProps>(function DialogContent(
  { className, overlayClassName, children, ...props },
  ref,
) {
  return (
    <BaseDialog.Portal>
      <Overlay className={overlayClassName} />
      <BaseDialog.Popup
        ref={ref}
        className={cn(
          "fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-elevated,var(--bg))] p-5 text-[var(--fg)] outline-none",
          "data-[ending-style]:opacity-0 data-[starting-style]:opacity-0 transition-opacity",
          className,
        )}
        {...props}
      >
        {children}
      </BaseDialog.Popup>
    </BaseDialog.Portal>
  );
});

const Title = React.forwardRef<
  HTMLHeadingElement,
  React.ComponentPropsWithoutRef<typeof BaseDialog.Title>
>(function DialogTitle({ className, ...props }, ref) {
  return (
    <BaseDialog.Title
      ref={ref}
      className={cn(
        "text-sm font-medium uppercase tracking-[0.12em] font-[var(--font-mono)] text-[var(--fg)]",
        className,
      )}
      {...props}
    />
  );
});

const Description = React.forwardRef<
  HTMLParagraphElement,
  React.ComponentPropsWithoutRef<typeof BaseDialog.Description>
>(function DialogDescription({ className, ...props }, ref) {
  return (
    <BaseDialog.Description
      ref={ref}
      className={cn("mt-1 text-xs leading-5 text-[var(--fg-muted,var(--fg))]/70", className)}
      {...props}
    />
  );
});

export const Dialog = Object.assign(Root, {
  Root,
  Trigger,
  Portal,
  Overlay,
  Content,
  Title,
  Description,
  Close,
});
