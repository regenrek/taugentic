import type { StyleDesc } from "@regenrek/gpuix-react"
import { forwardRef, type ForwardedRef, type ReactNode } from "react"

type PressableRole = "button" | "checkbox" | "menuitem" | "option" | "radio" | "tab" | "treeitem"
type PressableElement = { id: number }

export type PressableProps = {
  children: ReactNode
  name: string
  onPress(): void
  testId?: string
  disabled?: boolean
  role?: PressableRole
  selected?: boolean
  checked?: boolean
  expanded?: boolean
  style?: StyleDesc
}

function activates(event: { key?: string }): boolean {
  return event.key === "enter" || event.key === "space"
}

function setRef(ref: ForwardedRef<PressableElement>, element: PressableElement | null): void {
  if (typeof ref === "function") ref(element)
  else if (ref) ref.current = element
}

/** Desktop-owned semantic activation for Taugentic product controls. */
export const Pressable = forwardRef<PressableElement, PressableProps>(function Pressable({
  children,
  name,
  onPress,
  testId,
  disabled = false,
  role = "button",
  selected,
  checked,
  expanded,
  style,
}, ref) {
  const activate = () => {
    if (!disabled) onPress()
  }

  return <div
    ref={(instance) => setRef(ref, instance === null ? null : { id: instance.id })}
    testId={testId}
    tabIndex={disabled ? -1 : 0}
    accessibilityRole={role}
    accessibilityName={name}
    accessibilityDisabled={disabled}
    accessibilitySelected={selected}
    accessibilityChecked={checked}
    accessibilityExpanded={expanded}
    onClick={activate}
    onKeyDown={(event) => { if (activates(event)) activate() }}
    style={style}
  >{children}</div>
})
