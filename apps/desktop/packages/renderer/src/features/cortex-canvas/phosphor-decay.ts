/*
 * DOM-side phosphor-decay primitive.
 *
 * The canvas engine never paints log lines, so DOM consumers
 * (activity-log, run-stream, tool-output) opt into the decay by
 * applying this CSS class. The matching rule + keyframe lives in
 * styles/global.css and is driven by --mc-decay-ms.
 */

import type { CSSProperties } from "react";

export const PHOSPHOR_DECAY_CLASS = "mc-phosphor-decay";

export function phosphorDecayClass(): string {
  return PHOSPHOR_DECAY_CLASS;
}

export interface PhosphorDecayStyleArgs {
  ms?: number;
}

export function phosphorDecayStyle(args?: PhosphorDecayStyleArgs): CSSProperties {
  if (!args || args.ms == null) return {};
  const ms = Math.max(0, Math.floor(args.ms));
  return { ["--mc-decay-ms" as string]: `${ms}ms` } as CSSProperties;
}
