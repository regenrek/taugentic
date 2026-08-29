# Third-party notices

## Ported source

GPUIX's text selection, syntax highlighting, markdown renderer and diff viewer
are ported from **[Comet](https://github.com/zeronsh/comet)** (MIT, Copyright (c)
2026 Wing). The ported files carry a header naming their original.

| GPUIX file | Comet original |
| --- | --- |
| `packages/native/src/text/selection.rs` | `crates/ui/src/markdown/selection.rs` |
| `packages/native/src/text/paint.rs` | selection sections of `crates/ui/src/markdown/render.rs` |
| `packages/native/src/text/runs.rs` | `runs_for_syntax_line_with_plain` in `crates/ui/src/markdown/render.rs` |
| `packages/native/src/syntax/mod.rs` | `crates/syntax/src/lib.rs` |
| `packages/native/src/syntax/cache.rs` | `crates/ui/src/syntax_cache.rs` |
| `packages/native/src/markdown/parser.rs` | `crates/ui/src/markdown/parser.rs` |
| `packages/native/src/markdown/render.rs` | `crates/ui/src/markdown/render.rs` |
| `packages/native/src/diff/mod.rs` | pure sections of `crates/ui/src/changes.rs` |
| `packages/native/src/custom_elements/diff.rs` | rendering sections of `crates/ui/src/changes.rs` |
| `packages/native/src/custom_elements/code.rs` | `render_code_block` in `crates/ui/src/markdown/render.rs` |
| `packages/native/src/custom_elements/input.rs` | caret blink sections of `crates/ui/src/composer.rs` |
| `packages/native/src/theme.rs` | `crates/ui/src/theme.rs` |

## Example icons

The chat example uses **[Lucide](https://github.com/lucide-icons/lucide)** SVG
icons (ISC, Copyright (c) 2026 Lucide Icons and Contributors). The OpenAI mark
is ported from **[Comet](https://github.com/zeronsh/comet)** (MIT, Copyright (c)
2026 Wing).

## Bundled syntax definitions

Syntax highlighting uses **[Syntect](https://github.com/trishume/syntect)** (MIT)
with its pure-Rust **[fancy-regex](https://github.com/fancy-regex/fancy-regex)**
engine (MIT). Extra Sublime syntaxes (TypeScript, TSX, TOML, and others missing
from Syntect's default dump) come from **[two-face](https://codeberg.org/CosmicHarper/two-face)**,
the pack curated by [bat](https://github.com/sharkdp/bat). Versions are pinned
in `packages/native/Cargo.lock`.

The two-face crate is MIT OR Apache-2.0. The **embedded syntax files** have
their own licenses (Sublime, MIT, BSD, Apache, and others). The full listing
is in two-face's
[acknowledgements](https://codeberg.org/CosmicHarper/two-face/src/branch/main/generated/acknowledgements_full.md).

| Component | License | Source |
| --- | --- | --- |
| Syntect | MIT | https://github.com/trishume/syntect |
| fancy-regex | MIT | https://github.com/fancy-regex/fancy-regex |
| two-face crate | MIT OR Apache-2.0 | https://codeberg.org/CosmicHarper/two-face |

## Other dependencies

| Component | License | Source |
| --- | --- | --- |
| alacritty_terminal (Zed fork, pinned revision) | Apache-2.0 | https://github.com/zed-industries/alacritty |
| pulldown-cmark | MIT | https://github.com/pulldown-cmark/pulldown-cmark |
| arboard | MIT / Apache-2.0 | https://github.com/1Password/arboard |
| GPUI | Apache-2.0 | https://github.com/zed-industries/zed |

GPUIX itself is licensed under the terms in `LICENSE`.
