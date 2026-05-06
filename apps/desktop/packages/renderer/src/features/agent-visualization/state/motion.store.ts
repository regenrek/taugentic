/*
 * Motion store (xstate-store).
 *
 * Holds the manual cortex pause toggle for the visualization panel.
 * The store is intentionally small: it owns ONLY local UI state. Reading
 * canonical session/run/approval data still goes through TanStack Query
 * hooks, per the Mission Control drift rule "no second daemon-derived
 * domain owner". The cortex engine receives the paused flag as a prop
 * (the engine respects pause internally).
 */

import { createStore } from "@xstate/store";

export interface MotionContext {
  paused: boolean;
}

export type MotionSnapshot = { context: MotionContext };

export type MotionStore = ReturnType<typeof createMotionStore>;

export function createMotionStore() {
  return createStore({
    context: { paused: false } as MotionContext,
    on: {
      paused: (context) => ({ ...context, paused: true }),
      resumed: (context) => ({ ...context, paused: false }),
      toggled: (context) => ({ ...context, paused: !context.paused }),
      setPaused: (context, event: { paused: boolean }) => ({
        ...context,
        paused: event.paused,
      }),
    },
  });
}

export const motionStore: MotionStore = createMotionStore();

export function selectMotionPaused(snapshot: MotionSnapshot): boolean {
  return snapshot.context.paused;
}

export function setMotionPaused(paused: boolean, store: MotionStore = motionStore): void {
  store.trigger.setPaused({ paused });
}

export function toggleMotionPaused(store: MotionStore = motionStore): void {
  store.trigger.toggled();
}

export function getMotionPaused(store: MotionStore = motionStore): boolean {
  return store.getSnapshot().context.paused;
}
