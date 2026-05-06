import type { StreamUnsubscribe } from "../../lib/ipc/stream";

type StateUpdater<TState> = (state: TState) => TState;

type SnapshotRefreshResult<TCursor> = {
  afterCursor: TCursor | null;
  didHydrate: boolean;
};

export interface HydratedStreamConnectionDeps<TState, TMessage, TSnapshot, TCursor> {
  getAfterCursor(snapshot: TSnapshot): TCursor | null;
  hydrateSnapshot(state: TState, snapshot: TSnapshot): TState;
  loadSnapshot(): Promise<TSnapshot>;
  handleSnapshotError?(error: unknown): void;
  onDecodeError(state: TState): TState;
  onSnapshotError(state: TState, error: unknown): TState;
  subscribeStream(
    afterCursor: TCursor | null,
    onMessage: (message: TMessage) => void,
    onDecodeError: () => void,
  ): Promise<StreamUnsubscribe>;
  reduceMessage(state: TState, message: TMessage): { needsRefresh: boolean; state: TState };
}

export function connectHydratedStream<TState, TMessage, TSnapshot, TCursor>(
  setState: (updater: StateUpdater<TState>) => void,
  deps: HydratedStreamConnectionDeps<TState, TMessage, TSnapshot, TCursor>,
): () => void {
  let disposed = false;
  let unsubscribeStream: StreamUnsubscribe | null = null;
  let refreshPromise: Promise<SnapshotRefreshResult<TCursor>> | null = null;
  let refreshQueued = false;

  function refreshSnapshot(): Promise<SnapshotRefreshResult<TCursor>> {
    if (refreshPromise) {
      refreshQueued = true;
      return refreshPromise;
    }

    refreshQueued = false;

    const nextRefreshPromise = deps
      .loadSnapshot()
      .then((snapshot) => {
        if (disposed) {
          return { afterCursor: null, didHydrate: false };
        }

        setState((current) => deps.hydrateSnapshot(current, snapshot));
        return {
          afterCursor: deps.getAfterCursor(snapshot),
          didHydrate: true,
        };
      })
      .catch((error: unknown) => {
        if (disposed) {
          return { afterCursor: null, didHydrate: false };
        }

        deps.handleSnapshotError?.(error);
        setState((current) => deps.onSnapshotError(current, error));
        return { afterCursor: null, didHydrate: false };
      })
      .finally(() => {
        refreshPromise = null;
        if (!disposed && refreshQueued) {
          void refreshSnapshot();
        }
      });

    refreshPromise = nextRefreshPromise;
    return nextRefreshPromise;
  }

  void refreshSnapshot()
    .then((result) => {
      if (!result.didHydrate || disposed) {
        return null;
      }

      return deps.subscribeStream(
        result.afterCursor,
        (message) => {
          if (disposed) {
            return;
          }

          let needsRefresh = false;
          setState((current) => {
            const reduced = deps.reduceMessage(current, message);
            needsRefresh = reduced.needsRefresh;
            return reduced.state;
          });
          if (!disposed && needsRefresh) {
            void refreshSnapshot();
          }
        },
        () => {
          if (disposed) {
            return;
          }

          setState((current) => deps.onDecodeError(current));
        },
      );
    })
    .then((nextUnsubscribe) => {
      if (nextUnsubscribe === null || nextUnsubscribe === undefined) {
        return;
      }

      if (disposed) {
        nextUnsubscribe();
        return;
      }

      unsubscribeStream = nextUnsubscribe;
    })
    .catch((error: unknown) => {
      if (disposed) {
        return;
      }

      deps.handleSnapshotError?.(error);
      setState((current) => deps.onSnapshotError(current, error));
    });

  return () => {
    disposed = true;
    unsubscribeStream?.();
  };
}
