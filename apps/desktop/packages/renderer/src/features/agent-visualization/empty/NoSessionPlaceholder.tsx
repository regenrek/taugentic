/*
 * Empty-state placeholder for AgentVisualizationPanel.
 *
 * Rendered when no session is selected. Deliberately dim and centered so
 * the operator's eye is drawn to the left rail; CortexField is NOT mounted
 * in this state (the engine is expensive and there is nothing to visualize).
 */

export function NoSessionPlaceholder() {
  return (
    <div
      aria-label="Agent visualization"
      className="flex h-full w-full items-center justify-center"
      data-agent-visualization="empty"
      role="region"
    >
      <div className="px-4 py-6 text-center font-[var(--font-mono)] text-[12px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
        Select a session in the left rail to visualize its run.
      </div>
    </div>
  );
}
