export interface DisposableController {
  dispose(): void;
}

export interface ControllerRefSlot<T> {
  current: T | null;
}

export function getControllerRefValue<T>(slot: ControllerRefSlot<T>, create: () => T): T {
  if (slot.current === null) {
    slot.current = create();
  }

  return slot.current;
}

export function disposeControllerRefValue<T extends DisposableController>(
  slot: ControllerRefSlot<T>,
  controller: T,
): void {
  controller.dispose();
  if (slot.current === controller) {
    slot.current = null;
  }
}
