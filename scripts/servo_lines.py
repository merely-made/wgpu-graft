#!/usr/bin/env python3

# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

"""Report Servo's current release ladder, read straight from upstream refs.

The sync workflows used to ask the GitHub releases API which line to track,
which needed a token and depended on how upstream titles a release ("lts" in
the name). Branches and tags are the durable facts, so read those instead:

    release/v0.4  with tag v0.4.0   -> a shipped line
    release/v0.5  with no tag yet   -> the line currently in flight

Prints GitHub Actions `key=value` lines:

    released=release/v0.4    newest release branch that has a matching tag
    newest=release/v0.5      newest release branch, tagged or not
    upstream=<sha>           tip of upstream main

`released` and `newest` are equal whenever upstream has not yet branched the
next line; a caller syncing `newest` should treat that as nothing to do.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

SERVO = "https://github.com/servo/servo.git"

BRANCH = re.compile(r"^refs/heads/(release/v(\d+(?:\.\d+)*))$")
TAG = re.compile(r"^refs/tags/v(\d+(?:\.\d+)*)$")


def version(text: str) -> tuple[int, ...]:
    return tuple(int(part) for part in text.split("."))


def main() -> int:
    listing = subprocess.run(
        ["git", "ls-remote", "--heads", "--tags", SERVO],
        capture_output=True,
        text=True,
        check=True,
    ).stdout

    branches: dict[tuple[int, ...], str] = {}
    upstream = ""
    tags: list[tuple[int, ...]] = []

    for line in listing.splitlines():
        sha, _, ref = line.partition("\t")
        ref = ref.strip()
        if ref.endswith("^{}"):  # peeled annotated tag, same version
            continue
        if ref == "refs/heads/main":
            upstream = sha
        elif match := BRANCH.match(ref):
            branches[version(match.group(2))] = match.group(1)
        elif match := TAG.match(ref):
            tags.append(version(match.group(1)))

    if not branches:
        print("error: no release/vX.Y branches found upstream", file=sys.stderr)
        return 1

    newest = max(branches)
    # A line has shipped once a tag exists inside it: v0.4.0 sits in release/v0.4.
    shipped = [line for line in branches if any(tag[: len(line)] == line for tag in tags)]
    if not shipped:
        print("error: no release branch has a matching tag", file=sys.stderr)
        return 1
    released = max(shipped)

    out = [
        f"released={branches[released]}",
        f"newest={branches[newest]}",
        f"upstream={upstream}",
    ]
    for line in out:
        print(line)

    if path := os.environ.get("GITHUB_OUTPUT"):
        with open(path, "a", encoding="utf-8") as handle:
            handle.write("\n".join(out) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
