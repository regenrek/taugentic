import { palette } from "../app/theme.js"
import { Pressable } from "./pressable.js"

/** Presentation-only control: the caller owns the durable text it exposes. */
export function CopyTextButton(props: { text: string; copyText?(text: string): void; testId: string; label?: string }) {
  if (!props.text || !props.copyText) return null
  const label = props.label ?? "Copy"
  return <Pressable testId={props.testId} name={label} onPress={() => props.copyText?.(props.text)} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panelRaised }}><text style={{ color: palette.textMuted, fontSize: 10 }}>{label}</text></Pressable>
}
