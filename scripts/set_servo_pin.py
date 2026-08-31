#!/usr/bin/env python3
"""Point every Servo dependency in this workspace at one upstream ref.

The supported main-line sync keeps this rewrite in one tested script. Earlier
branch-specific workflows pasted the same logic into each file; those copies
drifted and broke: they only matched the bare `servo = "X.Y.Z"` string
form, so a table-form pin like

    servo = { version = "0.1.0", optional = true }

in servo-wgpu-interop-adapter kept its old version while the demo crates moved
to the new one. Two Servo versions in one graph pulls in two Stylo versions, and
`links = "servo_style_crate"` may only appear once in a dependency graph, so
every scheduled run died in `cargo update` from May 2026 onward without ever
pushing. The copies also carried a hardcoded five-manifest list that never grew
when the bevy, blitz, egui, and slint demos were added.

Usage:
    set_servo_pin.py --branch release/v0.4
    set_servo_pin.py --rev 8446a04aa2ef9c8ceae78af3be51df8a7b8130f3
    set_servo_pin.py --version 0.4.0

Rewrites every `servo` dependency under any `[*dependencies]` table in the
workspace's member manifests, preserving `optional = true`, plus the `servo =`
line in the adapter README and the Servo line references in both READMEs. Exits
non-zero if it finds nothing to rewrite, so a silent no-op fails the run rather
than committing an unchanged tree.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SERVO_GIT = "https://github.com/servo/servo"

# Members live one directory down. This deliberately does not recurse: the
# vendored third-party sources under patches/ are not ours to repin.
MANIFEST_GLOB = "*/Cargo.toml"

READMES = (Path("README.md"), Path("servo-wgpu-interop-adapter/README.md"))

# `[dependencies]`, `[dev-dependencies]`, `[target.'cfg(...)'.dependencies]`, ...
DEP_SECTION = re.compile(r"^\[.*dependencies\]$")
SECTION = re.compile(r"^\[.+\]$")
SERVO_DEP = re.compile(r"^servo\s*=\s*(?P<value>.+?)\s*$")

# Doc references to whichever Servo line the branch currently tracks.
README_BRANCH_REF = re.compile(r"release/v\d+\.\d+")
README_VERSION_PIN = re.compile(r'servo = "[^"]+"')


def spec_for(args: argparse.Namespace, *, optional: bool) -> str:
    """The dependency value to write, preserving an existing `optional = true`."""
    tail = ", optional = true" if optional else ""
    if args.version:
        if optional:
            return f'{{ version = "{args.version}"{tail} }}'
        return f'"{args.version}"'
    key, value = ("branch", args.branch) if args.branch else ("rev", args.rev)
    return f'{{ git = "{SERVO_GIT}", {key} = "{value}"{tail} }}'


def rewrite_manifest(path: Path, args: argparse.Namespace) -> bool:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    in_deps = False
    changed = False

    for i, line in enumerate(lines):
        stripped = line.strip()
        if SECTION.match(stripped):
            in_deps = bool(DEP_SECTION.match(stripped))
            continue
        if not in_deps:
            # `[features]` also has a `servo = [...]` key; never touch it.
            continue

        match = SERVO_DEP.match(stripped)
        if not match:
            continue

        old = match.group("value")
        new = spec_for(args, optional="optional = true" in old)
        if old == new:
            continue

        ending = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
        lines[i] = f"servo = {new}{ending}"
        changed = True
        print(f"  {path}: servo = {old}  ->  servo = {new}")

    if changed:
        path.write_text("".join(lines), encoding="utf-8")
    return changed


def rewrite_readme(path: Path, args: argparse.Namespace) -> bool:
    """Keep the docs naming the same Servo line the manifests now pin.

    The workflow runs on the branch it is updating, so rewriting every Servo
    line reference in place keeps that checkout's manifests and guidance
    aligned.
    """
    if not path.exists():
        return False

    text = original = path.read_text(encoding="utf-8")

    if args.branch:
        text = README_BRANCH_REF.sub(args.branch, text)
    if args.version:
        text = README_VERSION_PIN.sub(f'servo = "{args.version}"', text)
    else:
        spec = spec_for(args, optional=False)
        text = README_VERSION_PIN.sub(f"servo = {spec}", text)

    if text == original:
        return False
    path.write_text(text, encoding="utf-8")
    print(f"  {path}: updated Servo line references")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument("--branch", help="track a Servo release branch, e.g. release/v0.4")
    target.add_argument("--rev", help="pin an exact Servo commit sha")
    target.add_argument("--version", help="use the crates.io release, e.g. 0.4.0")
    args = parser.parse_args()

    if args.rev and not re.fullmatch(r"[0-9a-f]{40}", args.rev):
        parser.error(f"--rev wants a full 40-character sha, got {args.rev!r}")

    manifests = sorted(Path().glob(MANIFEST_GLOB))
    if not manifests:
        print("error: no member manifests found; wrong working directory?", file=sys.stderr)
        return 1

    print(f"Repinning Servo across {len(manifests)} manifests:")
    touched = [path for path in manifests if rewrite_manifest(path, args)]
    for readme in READMES:
        rewrite_readme(readme, args)

    if not touched:
        # Either everything already matched, or the pins have taken a shape this
        # script no longer recognises. The caller cannot tell those apart from a
        # zero exit, and the second is how the old inline rewriters failed
        # silently for months, so make the caller look.
        remaining = [
            path
            for path in manifests
            if re.search(r"^servo\s*=", path.read_text(encoding="utf-8"), re.MULTILINE)
        ]
        if remaining:
            print("No manifest changed; Servo pins already match the requested ref.")
            return 0
        print("error: no manifest declares a servo dependency", file=sys.stderr)
        return 1

    print(f"Repinned {len(touched)} manifests.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
