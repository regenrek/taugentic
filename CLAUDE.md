# Claude Notes

- Production-grade only. Keep one canonical implementation. Remove duplicate/dead paths.
- Cross-OS/distro work: use `crates/ta-host-platform` as SSOT.
- Keep diffs scoped; assume parallel edits.
- Ask before `git push`.
- Prefer deterministic commands, exact errors, latest docs, and end-to-end verify.
- ACP runtime changes must follow `docs/architecture/acp-runtime.md`.
