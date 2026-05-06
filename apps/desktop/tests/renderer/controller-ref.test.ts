import { describe, expect, it } from "vite-plus/test";

import {
  disposeControllerRefValue,
  getControllerRefValue,
  type ControllerRefSlot,
} from "../../packages/renderer/src/lib/react/controller-ref.js";

describe("controller ref lifecycle", () => {
  it("recreates a disposed controller on the next access", () => {
    const slot: ControllerRefSlot<TestController> = { current: null };
    let created = 0;

    const first = getControllerRefValue(slot, () => new TestController(++created));
    disposeControllerRefValue(slot, first);
    const second = getControllerRefValue(slot, () => new TestController(++created));

    expect(first.id).toBe(1);
    expect(first.disposeCalls).toBe(1);
    expect(slot.current).toBe(second);
    expect(second.id).toBe(2);
  });

  it("does not clear a newer controller when an older cleanup runs late", () => {
    const slot: ControllerRefSlot<TestController> = { current: null };
    const first = getControllerRefValue(slot, () => new TestController(1));

    slot.current = new TestController(2);
    const second = slot.current;
    disposeControllerRefValue(slot, first);

    expect(first.disposeCalls).toBe(1);
    expect(slot.current).toBe(second);
    expect(second.disposeCalls).toBe(0);
  });
});

class TestController {
  disposeCalls = 0;

  constructor(readonly id: number) {}

  dispose(): void {
    this.disposeCalls += 1;
  }
}
