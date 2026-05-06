# Architecture Decisions

Use this folder for numbered ADRs when a choice is expensive to reverse or would
otherwise drift into oral tradition.

## When to add an ADR

- the decision changes long-term ownership or layering
- the decision rejects a meaningful alternative
- future contributors will ask "why is it like this?"
- the choice has migration, compatibility, or operational consequences

## Naming

Use a stable numeric prefix and a short slug:

```text
0001-daemon-first-runtime.md
0002-electron-desktop-boundaries.md
```

## Suggested structure

- Context
- Decision
- Consequences
- Supersedes or superseded by

ADRs complement canonical current-state docs. They do not replace
`docs/architecture/`, `docs/contracts/`, or `docs/testing/`.
