#!/usr/bin/env python3
"""Resolve and prove that a staged consumer uses registry-only triplet crates."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


EXPECTED = {
    "grafting": "0.6.0",
    "scrying": "0.7.0",
    "welding": "0.14.0",
}

SERVO_RELEASE_SOURCE = (
    "git+https://github.com/servo/servo"
    "?rev=1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019#"
    "1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019"
)
GLSLOPT_FIX_SOURCE = (
    "git+https://github.com/jamienicol/glslopt-rs"
    "?rev=68019f02a1437d785408ebc8ce7fd20341fffb8a#"
    "68019f02a1437d785408ebc8ce7fd20341fffb8a"
)


def run(command: list[str], cwd: Path) -> str:
    completed = subprocess.run(command, cwd=cwd, check=True, text=True, stdout=subprocess.PIPE)
    return completed.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--consumer", type=Path, required=True)
    parser.add_argument("--receipt-dir", type=Path, required=True)
    parser.add_argument("--grafting-version", default=EXPECTED["grafting"])
    parser.add_argument("--scrying-version", default=EXPECTED["scrying"])
    parser.add_argument("--welding-version", default=EXPECTED["welding"])
    parser.add_argument(
        "--allow-servo-git",
        action="store_true",
        help="allow only Servo's recorded release/v0.5 commit and its pinned glslopt fix as Graft test scaffolding",
    )
    args = parser.parse_args()

    consumer = args.consumer.resolve()
    receipt_dir = args.receipt_dir.resolve()
    if not (consumer / "Cargo.toml").is_file():
        parser.error(f"not a Cargo consumer: {consumer}")
    receipt_dir.mkdir(parents=True, exist_ok=True)
    expected = {
        "grafting": args.grafting_version,
        "scrying": args.scrying_version,
        "welding": args.welding_version,
    }

    run(["cargo", "generate-lockfile"], consumer)
    metadata_text = run(["cargo", "metadata", "--locked", "--format-version", "1"], consumer)
    tree_text = run(["cargo", "tree", "--locked", "--workspace", "--edges", "normal"], consumer)
    (receipt_dir / "metadata.json").write_text(metadata_text, encoding="utf-8")
    (receipt_dir / "cargo-tree.txt").write_text(tree_text, encoding="utf-8")
    lockfile = consumer / "Cargo.lock"
    (receipt_dir / "Cargo.lock").write_text(lockfile.read_text(encoding="utf-8"), encoding="utf-8")
    receipt_facts = {
        "grafting_version": args.grafting_version,
        "scrying_version": args.scrying_version,
        "welding_version": args.welding_version,
        "graft_source_sha": os.environ.get("GITHUB_SHA", "local"),
        "scry_source_ref": os.environ.get("SCRY_SOURCE_REF", "local"),
        "weld_source_ref": os.environ.get("WELD_SOURCE_REF", "local"),
        "runner_name": os.environ.get("RUNNER_NAME", "local"),
        "runner_os": os.environ.get("RUNNER_OS", sys.platform),
        "wgpu_backend": os.environ.get("WGPU_BACKEND", "unspecified"),
    }
    (receipt_dir / "release-inputs.json").write_text(
        json.dumps(receipt_facts, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    metadata = json.loads(metadata_text)
    local_ids = set(metadata["workspace_members"])
    problems: list[str] = []
    allowed_non_registry: list[str] = []
    for package in metadata["packages"]:
        source = package.get("source")
        if package["id"] in local_ids or (source or "").startswith("registry+"):
            continue
        if args.allow_servo_git and (
            source == SERVO_RELEASE_SOURCE
            or source == GLSLOPT_FIX_SOURCE
        ):
            allowed_non_registry.append(
                f"{package['name']} {package['version']} ({source})"
            )
        else:
            problems.append(f"non-registry dependency: {package['name']} {package['version']} ({source})")

    (receipt_dir / "allowed-non-registry.txt").write_text(
        "\n".join(sorted(allowed_non_registry)) + ("\n" if allowed_non_registry else ""),
        encoding="utf-8",
    )

    for name, version in expected.items():
        matches = [p for p in metadata["packages"] if p["name"] == name and p["version"] == version]
        if len(matches) != 1:
            problems.append(f"expected exactly one {name} {version}, found {len(matches)}")
            continue
        source = matches[0].get("source") or ""
        if not source.startswith("registry+"):
            problems.append(f"{name} {version} is not registry sourced: {source or 'local path'}")
        else:
            print(f"registry proof: {name} {version} <- {source}")

    if problems:
        for problem in problems:
            print(f"registry proof: FAIL: {problem}", file=sys.stderr)
        return 1
    print(f"registry proof: PASS ({consumer})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
