# Licenses in this repository

**This repository: MPL-2.0.** Every file Mark wrote carries the SPDX tag
`MPL-2.0`, per the
[license posture brief](../mere/design_docs/2026-08-22_license_posture_brief.md)
of 2026-08-22 (mere `design_docs/2026-08-22_license_posture_brief.md`). The
full text is in [`LICENSE`](LICENSE), and [`NOTICE`](NOTICE) carries the
upstream attribution this workspace was founded on.

This file is the provenance ledger. It is the authority for what the relicense
tool (mere `scripts/relicense_headers.py`) skips: the backtick-quoted paths in
the **Retained licenses** table are never touched. Provenance comes before
license — a file gets Exhibit A only if Mark wrote it.

## What this repository is made of

wgpu-graft is a small owned workspace sitting on top of a large vendored
`patches/` tree. **The retained part is most of the repository: 460 of 582
tracked files (79%), and 361 of 418 tracked source files (86%).** The whole of
`patches/` except `freetype-sys-compat` is other people's code, consumed
through `[patch]` and kept verbatim but for the build fixes each subdirectory's
own notes record. Mark's own part is 57 source files: the `grafting` crate,
`servo-wgpu-interop-adapter`, `demo-support`, the nine `demo-*` crates, the
`patches/freetype-sys-compat` shim, and `scripts/`.

The sweep plan's P7 item 4 anticipated this shape (it guessed a wgpu fork; the
vendored majority is in fact Zed's GPUI) and rules that where the ledger turns
out to be most of the repository, the ledger is written and the header pass is
left. That is the disposition here: this file is committed alone, and no source
file's header was changed.

## Retained licenses

Third-party code keeps its own license and its own notices. Nothing here is
relicensed, and nothing here receives a Merely copyright line.

| Path | License | Upstream | Notice files |
|---|---|---|---|
| `patches/glass-gpui` | Apache-2.0 | [glass-hq/gpui](https://github.com/glass-hq/gpui), a fork of [zed-industries/zed](https://github.com/zed-industries/zed)'s GPUI; `Copyright 2022 - 2025 Zed Industries, Inc.` | `LICENSE-APACHE` in eleven member crates; `assets/fonts/ibm-plex-sans/license.txt` (SIL OFL 1.1, IBM Plex) and `assets/fonts/lilex/OFL.txt` (SIL OFL 1.1, Lilex) for the bundled fonts |
| `patches/taffy-0.9` | MIT | [DioxusLabs/taffy](https://github.com/DioxusLabs/taffy) 0.9.2, republished as 0.9.0 to satisfy GPUI's exact pin | its manifest; upstream's `LICENSE.md` |
| `patches/serde_fmt` | Apache-2.0 OR MIT | [KodrAus/serde_fmt](https://github.com/KodrAus/serde_fmt) 1.1.0; `Copyright (c) 2019 Ashley Mannix` | `LICENSE-APACHE`, `LICENSE-MIT` in-tree |
| `patches/yeslogic-fontconfig-sys` | MIT | [yeslogic/fontconfig-rs](https://github.com/yeslogic/fontconfig-rs) 6.0.0 | its manifest |

460 tracked files. `patches/glass-gpui` also contains three files carrying
`Copyright (c) Microsoft Corporation.` (Windows API shims inside GPUI); those
travel with GPUI under its own terms and are not separated out.

`patches/freetype-sys-compat` is deliberately **not** in this table. It is
Mark's own six-line shim — a `pub use freetype_sys_upstream::*;` that satisfies
`zed-font-kit`'s 0.20 version edge without declaring a native `links` key — and
contains no upstream code. Its manifest says `license = "MIT"` to sit beside
the crate it stands in for; P7 changes headers only, so that line was not
touched and is recorded here as a known inconsistency for a later pass.

## Derivatives carrying MPL-2.0 with an upstream notice retained

These are **not** skipped. Each is Mark's substantial work over an upstream
starting point, relicensed MPL-2.0 with the upstream notice kept verbatim.
Apply with `--retain-notice` when the header pass is eventually run.

| Path | Upstream | Notices kept |
|---|---|---|
| `grafting`, `servo-wgpu-interop-adapter` | the [Slint Servo embedding example](https://github.com/slint-ui/slint/tree/master/examples/servo), MIT OR Apache-2.0 | `Copyright (c) SixtyFPS GmbH <info@slint.dev>` in the root [`NOTICE`](NOTICE), which is the notice file for both crates; no per-file upstream copyright line exists to preserve |
| `demo-servo-blitz/src/keyutils.rs`, `demo-servo-winit/src/keyutils.rs`, `demo-servo-xilem/src/keyutils.rs` | [servo/servo](https://github.com/servo/servo) `servoshell/desktop/keyutils.rs`, MPL-2.0 | each file's leading `// Adapted from Servo's servoshell/desktop/keyutils.rs` / `// Original: Mozilla Public License 2.0` provenance pair |

Those three `keyutils.rs` are the only owned files the sweep's
`git grep -l 'Mozilla Public'` finds today, and their match is that provenance
note rather than Exhibit A. A header pass must not read it as an existing
Exhibit A, and must not delete it.

**This section is deliberately not the skip list.** The tool reads only the
`## Retained licenses` table above.

## Exceptions under the fork/vendor criterion

**None.** The brief's §4 test — a crate stays MIT OR Apache-2.0 only when a
third party would need to *modify or vendor* it rather than merely link it —
admits nothing in this repository. `grafting` is published (0.5.1) under
MPL-2.0 already; the published versions of every owned crate keep the grant
they shipped with, per the sweep plan's invariant 8.

## How to add a file from elsewhere

1. Do not delete or rewrite the upstream copyright or license notice, ever.
2. Add its path to **Retained licenses** above with its license, upstream URL,
   and where its notice text lives. The tool then skips it automatically.
3. If it is a substantial derivative rather than a verbatim import, the brief's
   rule is MPL-2.0 on the derivative *with the upstream notice retained* —
   record it in that section so the distinction is not lost.
4. Never add `license-file` to an owned manifest.
5. Re-run `python ../mere/scripts/relicense_headers.py --repo . --audit` and
   confirm the owned source count moved by exactly what you expected.
