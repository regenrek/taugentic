import type { StyleDesc } from "@regenrek/gpuix-react"
import type { ReactNode } from "react"

type PressableRole = "button" | "checkbox" | "menuitem" | "option" | "radio" | "tab" | "treeitem"

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

/** Desktop-owned semantic activation for Taugentic product controls. */
export function Pressable({
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
}: PressableProps) {
  const activate = () => {
    if (!disabled) onPress()
  }

  return <div
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
}
