import type { SessionId, WorkItem } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import {
  useDismissWorkItemMutation,
  useRefreshWorkItemsMutation,
  useTriggerWorkItemMutation,
  useWorkItemsQuery,
} from "@/lib/queries/work-items";

export interface WorkInboxProps {
  selectedSessionId: SessionId | null;
}

export function WorkInbox({ selectedSessionId }: WorkInboxProps) {
  const workItems = useWorkItemsQuery();
  const refreshMutation = useRefreshWorkItemsMutation();
  const dismissMutation = useDismissWorkItemMutation();
  const triggerMutation = useTriggerWorkItemMutation(selectedSessionId);
  const items = workItems.data?.items ?? [];
  const errorMessage =
    workItems.error ?? refreshMutation.error ?? dismissMutation.error ?? triggerMutation.error;
  const isBusy =
    workItems.isFetching ||
    refreshMutation.isPending ||
    dismissMutation.isPending ||
    triggerMutation.isPending;

  return (
    <section className="flex flex-col gap-2 border-b border-[var(--border)] px-3 py-3">
      <div className="flex items-center gap-2">
        <h2 className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--fg-dim)]">
          work inbox
        </h2>
        <span className="rounded border border-[var(--border)] px-1.5 py-[1px] font-[var(--font-mono)] text-[10px] text-[var(--fg-mute)]">
          {items.length}
        </span>
        <Button
          className="ml-auto h-6 px-2 text-[10px]"
          disabled={refreshMutation.isPending}
          onClick={() => refreshMutation.mutate({})}
          size="sm"
          type="button"
          variant="ghost"
        >
          {refreshMutation.isPending ? "refreshing" : "refresh"}
        </Button>
      </div>
      {workItems.isLoading ? (
        <p className="text-[12px] text-[var(--fg-mute)]" role="status">
          Loading work items...
        </p>
      ) : null}
      {errorMessage ? (
        <p className="text-[12px] text-[var(--destructive-foreground,#ff6b6b)]" role="alert">
          {errorMessage instanceof Error ? errorMessage.message : String(errorMessage)}
        </p>
      ) : null}
      {!workItems.isLoading && items.length === 0 && !errorMessage ? (
        <p className="text-[12px] text-[var(--fg-mute)]" role="status">
          No background work items.
        </p>
      ) : null}
      {items.length > 0 ? (
        <div className="flex max-h-52 flex-col gap-2 overflow-y-auto pr-1">
          {items.map((item) => (
            <WorkInboxRow
              disabled={isBusy}
              item={item}
              key={item.key}
              onDismiss={() => dismissMutation.mutate({ key: item.key })}
              onTrigger={() => triggerMutation.mutate({ key: item.key })}
              selectedSessionId={selectedSessionId}
            />
          ))}
        </div>
      ) : null}
      {workItems.data?.sync.detail ? (
        <p className="text-[10px] text-[var(--fg-mute)]">{workItems.data.sync.detail}</p>
      ) : null}
    </section>
  );
}

function WorkInboxRow({
  disabled,
  item,
  onDismiss,
  onTrigger,
  selectedSessionId,
}: {
  disabled: boolean;
  item: WorkItem;
  onDismiss: () => void;
  onTrigger: () => void;
  selectedSessionId: SessionId | null;
}) {
  const canTrigger = item.status === "available" && selectedSessionId !== null;
  return (
    <article className="rounded border border-[var(--border)] bg-[var(--panel)]/40 p-2">
      <div className="flex items-center gap-2 text-[11px]">
        <span className="font-[var(--font-mono)] text-[var(--fg-mute)]">{item.externalId}</span>
        <span className="ml-auto rounded border border-[var(--border)] px-1.5 py-[1px] uppercase tracking-[0.12em] text-[9px] text-[var(--fg-dim)]">
          {item.status}
        </span>
      </div>
      <a
        className="mt-1 block truncate text-[12px] text-[var(--fg)] hover:underline"
        href={item.url}
        rel="noreferrer"
        target="_blank"
      >
        {item.title}
      </a>
      <p className="mt-1 line-clamp-2 text-[11px] text-[var(--fg-dim)]">
        {item.body || "(no body)"}
      </p>
      <div className="mt-2 flex gap-2">
        <Button disabled={disabled || !canTrigger} onClick={onTrigger} size="sm" type="button">
          trigger
        </Button>
        <Button disabled={disabled} onClick={onDismiss} size="sm" type="button" variant="ghost">
          dismiss
        </Button>
      </div>
    </article>
  );
}
