import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";

import type { ApprovalStreamMessage, SessionId } from "@taugentic/desktop-shared";

import { subscribeApprovalStream } from "@/lib/ipc/stream";
import { useMountEffect } from "@/lib/react/use-mount-effect";
import { queryKeys } from "@/lib/queries/keys";
import {
  createInitialSessionApprovalState,
  reduceApprovalStreamMessage,
  toApprovalStreamErrorMessage,
  type SessionApprovalState,
} from "@/features/approvals/stream-state";

export interface SessionApprovalLiveSyncView {
  readonly streamStatus: SessionApprovalState["streamStatus"];
  readonly errorMessage: SessionApprovalState["errorMessage"];
  readonly lastSequence: SessionApprovalState["lastSequence"];
}

export interface ApprovalStreamStepResult {
  readonly nextState: SessionApprovalState;
  readonly nextView: SessionApprovalLiveSyncView;
  readonly shouldInvalidate: boolean;
}

/**
 * Pure step: fold a stream message into state + view + invalidation decision.
 *
 * Extracted so the orchestration wiring in {@link useSessionApprovalLiveSync}
 * remains a thin adapter over testable logic.
 */
export function applyApprovalStreamMessage(
  state: SessionApprovalState,
  message: ApprovalStreamMessage,
): ApprovalStreamStepResult {
  const { needsRefresh, state: nextState } = reduceApprovalStreamMessage(state, message);
  return {
    nextState,
    nextView: toView(nextState),
    shouldInvalidate: needsRefresh,
  };
}

/**
 * Pure step for stream error paths (both the subscriber error callback and a
 * failed initial `subscribeApprovalStream()` promise).
 */
export function applyApprovalStreamError(
  state: SessionApprovalState,
  sessionId: SessionId,
  error: unknown,
): ApprovalStreamStepResult {
  const nextState: SessionApprovalState = {
    ...state,
    errorMessage: toApprovalStreamErrorMessage(sessionId, error),
    streamStatus: "error",
  };
  return {
    nextState,
    nextView: toView(nextState),
    shouldInvalidate: false,
  };
}

function toView(state: SessionApprovalState): SessionApprovalLiveSyncView {
  return {
    streamStatus: state.streamStatus,
    errorMessage: state.errorMessage,
    lastSequence: state.lastSequence,
  };
}

/**
 * Mount-scoped approval-stream subscription for a session.
 *
 * Subscribes to the lane-agnostic daemon approval stream, tracks stream
 * status / error / last-sequence locally, and invalidates the shared
 * `sessionApprovals` query whenever the daemon emits an approval event.
 *
 * The component using this hook is expected to unmount/remount on sessionId
 * change (see `SessionDetailSurface` which keys on sessionId), so the mount
 * effect is sufficient.
 */
export function useSessionApprovalLiveSync(sessionId: SessionId): SessionApprovalLiveSyncView {
  const qc = useQueryClient();
  const [view, setView] = useState<SessionApprovalLiveSyncView>(() =>
    toView(createInitialSessionApprovalState(sessionId)),
  );
  const stateRef = useRef<SessionApprovalState>(createInitialSessionApprovalState(sessionId));
  const queryKey = useMemo(() => queryKeys.sessionApprovals(sessionId), [sessionId]);

  useMountEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | null = null;

    const applyStep = (step: ApprovalStreamStepResult): void => {
      stateRef.current = step.nextState;
      setView(step.nextView);
      if (step.shouldInvalidate) {
        void qc.invalidateQueries({ queryKey });
      }
    };

    void subscribeApprovalStream(
      sessionId,
      null,
      (message) => {
        if (disposed) {
          return;
        }
        applyStep(applyApprovalStreamMessage(stateRef.current, message));
      },
      (error) => {
        if (disposed) {
          return;
        }
        applyStep(applyApprovalStreamError(stateRef.current, sessionId, error));
      },
    )
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unsubscribe = dispose;
      })
      .catch((error: unknown) => {
        if (disposed) {
          return;
        }
        applyStep(applyApprovalStreamError(stateRef.current, sessionId, error));
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  });

  return view;
}
