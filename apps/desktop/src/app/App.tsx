import { useState } from "react"

import { metrics, palette } from "./theme.js"

const navItems = ["Sessions", "Work inbox", "Approvals"] as const
type NavItem = (typeof navItems)[number]

interface NavigationItemProps {
  item: NavItem
  selected: boolean
  onActivate(item: NavItem): void
}

function NavigationItem({ item, selected, onActivate }: NavigationItemProps) {
  return (
    <div
      testId={`nav-${item.toLowerCase().replace(" ", "-")}`}
      tabIndex={0}
      onClick={() => onActivate(item)}
      onKeyDown={(event) => {
        if (event.key === "enter") onActivate(item)
      }}
      style={{
        padding: 10,
        paddingLeft: 12,
        borderRadius: 7,
        cursor: "pointer",
        backgroundColor: selected ? palette.panelRaised : palette.panel,
        borderWidth: selected ? 1 : 0,
        borderColor: palette.borderStrong,
        hover: { backgroundColor: palette.panelRaised },
      }}
    >
      <text
        style={{
          color: selected ? palette.text : palette.textMuted,
          fontSize: 13,
          fontWeight: selected ? 600 : 450,
        }}
      >
        {item}
      </text>
    </div>
  )
}

export function App() {
  const [selectedNav, setSelectedNav] = useState<NavItem>("Sessions")

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: palette.canvas,
        color: palette.text,
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          height: metrics.titlebarHeight,
          paddingLeft: 86,
          paddingRight: 18,
          userSelect: "none",
        }}
      >
        <text style={{ color: palette.text, fontSize: 13, fontWeight: 650 }}>TAUGENTIC</text>
        <div style={{ flexGrow: 1 }} />
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            gap: 7,
            padding: 7,
            paddingLeft: 10,
            paddingRight: 10,
            borderRadius: 999,
            backgroundColor: palette.accentDim,
          }}
        >
          <div
            style={{ width: 7, height: 7, borderRadius: 999, backgroundColor: palette.accent }}
          />
          <text style={{ color: palette.accent, fontSize: 11, fontWeight: 600 }}>
            GPUI RUNTIME
          </text>
        </div>
      </div>
      <div style={{ height: 1, backgroundColor: palette.border }} />

      <div style={{ display: "flex", flexDirection: "row", flexGrow: 1, minHeight: 0 }}>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            width: metrics.sidebarWidth,
            padding: 14,
            gap: 5,
            backgroundColor: palette.panel,
            userSelect: "none",
          }}
        >
          <text
            style={{
              color: palette.textFaint,
              fontSize: 10,
              fontWeight: 700,
              marginBottom: 7,
            }}
          >
            WORKSPACE
          </text>
          {navItems.map((item) => (
            <NavigationItem
              key={item}
              item={item}
              selected={item === selectedNav}
              onActivate={setSelectedNav}
            />
          ))}
        </div>
        <div style={{ width: 1, backgroundColor: palette.border }} />

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            flexGrow: 1,
            minWidth: 0,
            padding: 28,
            gap: 18,
          }}
        >
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <text style={{ color: palette.text, fontSize: 23, fontWeight: 650 }}>{selectedNav}</text>
            <text style={{ color: palette.textMuted, fontSize: 13 }}>
              Native desktop foundation is ready for the daemon-backed vertical slice.
            </text>
          </div>

          <div
            style={{
              display: "flex",
              flexDirection: "column",
              maxWidth: 720,
              padding: 22,
              gap: 13,
              borderWidth: 1,
              borderColor: palette.border,
              borderRadius: metrics.panelRadius,
              backgroundColor: palette.panel,
            }}
          >
            <text style={{ color: palette.text, fontSize: 14, fontWeight: 650 }}>
              Desktop ownership
            </text>
            <text style={{ color: palette.textMuted, fontSize: 13, lineHeight: 20 }}>
              GPUIX owns native rendering and input. Taugentic owns product state. The daemon remains
              the only owner of sessions, runs, permissions, persistence, and harness execution.
            </text>
            <div
              style={{
                alignSelf: "flex-start",
                padding: 7,
                paddingLeft: 10,
                paddingRight: 10,
                borderRadius: 6,
                backgroundColor: palette.accentDim,
              }}
            >
              <text style={{ color: palette.accent, fontSize: 11, fontWeight: 650 }}>
                M0 · NATIVE HOST ACTIVE
              </text>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
