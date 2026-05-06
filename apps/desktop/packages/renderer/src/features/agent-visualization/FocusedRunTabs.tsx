/*
 * FocusedRunTabs.
 *
 * Five-tab assembly (Steps / Tool Calls / Diff / Metrics / Raw) built on
 * the new `Tabs` primitive (Base UI). Each panel renders a thin wrapper
 * around the canonical SessionDetail sub-component for that section, per
 * the Mission Control drift rule "tabs wrap existing components, do not
 * re-implement".
 */

import type { SessionId } from "@taugentic/desktop-shared";

import { Tabs } from "@/components/ui/tabs";

import { DiffTab } from "./tabs/DiffTab";
import { MetricsTab } from "./tabs/MetricsTab";
import { RawTab } from "./tabs/RawTab";
import { StepsTab } from "./tabs/StepsTab";
import { ToolCallsTab } from "./tabs/ToolCallsTab";

export type FocusedRunTabValue = "steps" | "tools" | "diff" | "metrics" | "raw";

const FOCUSED_RUN_TAB_DEFAULT: FocusedRunTabValue = "steps";

export interface FocusedRunTabsProps {
  sessionId: SessionId;
  onRunStarted?: () => void;
  /** Optional controlled value; when unset, the panel manages its own. */
  value?: FocusedRunTabValue;
  defaultValue?: FocusedRunTabValue;
  onValueChange?: (value: FocusedRunTabValue) => void;
}

export function FocusedRunTabs({
  defaultValue,
  onRunStarted,
  onValueChange,
  sessionId,
  value,
}: FocusedRunTabsProps) {
  const rootProps =
    value !== undefined
      ? {
          value,
          onValueChange: (next: unknown) => onValueChange?.(next as FocusedRunTabValue),
        }
      : {
          defaultValue: defaultValue ?? FOCUSED_RUN_TAB_DEFAULT,
          onValueChange: onValueChange
            ? (next: unknown) => onValueChange(next as FocusedRunTabValue)
            : undefined,
        };

  return (
    <Tabs.Root
      className="flex min-h-0 flex-1 flex-col gap-0"
      data-agent-visualization-tabs
      {...rootProps}
    >
      <Tabs.List className="px-2" variant="line">
        <Tabs.Trigger value="steps" variant="line">
          Steps
        </Tabs.Trigger>
        <Tabs.Trigger value="tools" variant="line">
          Tool Calls
        </Tabs.Trigger>
        <Tabs.Trigger value="diff" variant="line">
          Diff
        </Tabs.Trigger>
        <Tabs.Trigger value="metrics" variant="line">
          Metrics
        </Tabs.Trigger>
        <Tabs.Trigger value="raw" variant="line">
          Raw
        </Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content className="min-h-0 flex-1 overflow-auto" value="steps">
        <StepsTab onRunStarted={onRunStarted} sessionId={sessionId} />
      </Tabs.Content>
      <Tabs.Content className="min-h-0 flex-1 overflow-auto" value="tools">
        <ToolCallsTab sessionId={sessionId} />
      </Tabs.Content>
      <Tabs.Content className="min-h-0 flex-1 overflow-auto" value="diff">
        <DiffTab sessionId={sessionId} />
      </Tabs.Content>
      <Tabs.Content className="min-h-0 flex-1 overflow-auto" value="metrics">
        <MetricsTab sessionId={sessionId} />
      </Tabs.Content>
      <Tabs.Content className="min-h-0 flex-1 overflow-auto" value="raw">
        <RawTab sessionId={sessionId} />
      </Tabs.Content>
    </Tabs.Root>
  );
}
