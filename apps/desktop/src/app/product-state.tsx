import type { ReactNode } from "react"

import { palette } from "./theme.js"

/** Stateless semantic presentation of already-canonical product facts. */
export type ProductStateKind = "empty" | "loading" | "offline" | "error" | "destructive"

export function ProductState({ kind, title, detail, action }: {
  kind: ProductStateKind
  title: string
  detail?: string
  action?: ReactNode
}) {
  const role = kind === "error" ? "alert" : kind === "destructive" ? "alertdialog" : "status"
  const color = kind === "error" || kind === "destructive" ? "#F08080" : kind === "offline" ? palette.warning : palette.textMuted
  return <div testId={`product-state-${kind}`} accessibilityRole={role} accessibilityName={title} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10, padding: 18 }}>
    <text style={{ color, fontSize: 14, fontWeight: 650 }}>{title}</text>
    {detail && <text style={{ color: palette.textMuted, fontSize: 12 }}>{detail}</text>}
    {action}
  </div>
}
