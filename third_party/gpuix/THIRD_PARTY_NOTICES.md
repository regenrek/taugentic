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

## Browser compositor contract

The nested GPUI macOS Base / Native / Overlay compositor contract is adapted
from Zed's [GPUI PR #61945](https://github.com/zed-industries/zed/pull/61945)
at pinned commit
[`20a699acac7b5bceea8e8fe6ba257a61ad47fb09`](https://github.com/zed-industries/zed/commit/20a699acac7b5bceea8e8fe6ba257a61ad47fb09).
The adapted `crates/gpui`, `crates/gpui_apple`, and `crates/gpui_macos`
packages each declare **Apache-2.0**. GPUIX retains this notice for the
adapted compositor contract.

Copyright 2022-2025 Zed Industries, Inc.

The GPUIX fork modifies these Apache-2.0 files:

| File | GPUIX modification |
| --- | --- |
| `zed/crates/gpui/src/platform.rs` | Defines the native-surface platform contract. |
| `zed/crates/gpui/src/scene.rs` | Splits base and overlay scene ranges. |
| `zed/crates/gpui/src/window.rs` | Draws the layered scene. |
| `zed/crates/gpui_apple/src/metal_renderer.rs` | Adds the transparent overlay renderer. |
| `zed/crates/gpui_macos/src/window.rs` | Hosts native views between GPUI render planes. |

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
