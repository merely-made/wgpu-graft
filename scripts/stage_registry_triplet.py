#!/usr/bin/env python3
"""Stage a sibling demo as a standalone, registry-only triplet consumer.

The source tree is a *demo fixture*, not a library dependency.  This script
copies its source into a new directory and writes a manifest which names the
three released crates by exact version.  The resulting Cargo graph therefore
cannot silently use a sibling checkout, workspace dependency, patch, or Git
revision for grafting, scrying, or welding.

Run ``scripts/verify_registry_triplet.py`` in the staged directory before any
headed battery.  The two scripts are deliberately separate: staging describes
the intended consumer, while verification records what Cargo actually picked.
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path


KIND_TO_DEMO = {
    "scry-win": "demo-win",
    "scry-mac": "demo-mac",
    "scry-wpe": "demo-wpe",
    "weld-win": "demo-weld-win",
    "weld-linux": "demo-weld-linux",
    "weld-mac": "demo-weld-mac",
}


def exact_dependency(name: str, version: str, *, features: list[str] | None = None,
                     default_features: bool | None = None) -> str:
    fields = [f'version = "={version}"']
    if default_features is not None:
        fields.append(f"default-features = {str(default_features).lower()}")
    if features:
        joined = ", ".join(f'"{feature}"' for feature in features)
        fields.append(f"features = [{joined}]")
    return f"{name} = {{ {', '.join(fields)} }}"


def base_dependencies(grafting_version: str, scrying_version: str,
                      welding_version: str, *, active: str) -> list[str]:
    dependencies = [
        exact_dependency("grafting", grafting_version, features=["wgpu-30"], default_features=False),
    ]
    if active == "scry":
        dependencies.extend(
            [
                exact_dependency("scrying", scrying_version),
                exact_dependency("welding", welding_version, default_features=False),
            ]
        )
    else:
        dependencies.extend(
            [
                exact_dependency("scrying", scrying_version, default_features=False),
                exact_dependency("welding", welding_version, features=["cef-runtime"]),
            ]
        )
    return dependencies


def manifest_for(kind: str, grafting_version: str, scrying_version: str,
                 welding_version: str) -> str:
    active = "scry" if kind.startswith("scry-") else "weld"
    dependencies = base_dependencies(grafting_version, scrying_version, welding_version, active=active)
    package_name = f"registry-triplet-{kind}"
    header = f'''[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"
publish = false

'''
    binary_name = KIND_TO_DEMO[kind]
    binary = f'''[[bin]]
name = "{binary_name}"
path = "src/main.rs"

'''

    if kind == "scry-win":
        dependencies.extend(
            [
                'pollster = "0.4.0"',
                'wgpu = { version = "=30.0.1", features = ["metal"] }',
                'winit = "=0.30.13"',
            ]
        )
        return header + binary + "[dependencies]\n" + "\n".join(dependencies) + '''

[target.'cfg(windows)'.dependencies]
raw-window-handle = "=0.6.2"
webview2-com = "=0.39.1"
windows = { version = "=0.62.2", features = [
    "Foundation", "System", "UI", "UI_Composition", "UI_Composition_Core",
    "UI_Composition_Desktop", "Win32_Foundation",
    "Win32_System_Com_StructuredStorage", "Win32_System_Memory",
    "Win32_System_Threading", "Win32_System_WinRT",
    "Win32_System_WinRT_Composition", "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
'''

    if kind == "scry-mac":
        dependencies.extend(
            [
                'image = "0.25"',
                'pollster = "0.4.0"',
                'raw-window-handle = "=0.6.2"',
                'wgpu = { version = "=30.0.1", features = ["metal"] }',
                'winit = "=0.30.13"',
            ]
        )
        return header + binary + "[dependencies]\n" + "\n".join(dependencies) + '''

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "=0.6.3"
objc2-app-kit = "=0.3.2"
'''

    if kind == "scry-wpe":
        dependencies.extend(
            [
                'dpi = "0.1.2"',
                'glib = "=0.18.5"',
                'libc = "0.2"',
                exact_dependency("scrying", scrying_version, features=["wpe"]),
            ]
        )
        # Replace the inactive scrying dependency with the WPE-enabled one.
        dependencies = [line for line in dependencies if line != exact_dependency("scrying", scrying_version)]
        return header + binary + '''[features]
wpe = []

[dependencies]
''' + "\n".join(dependencies) + "\n"

    dependencies.extend(
        [
            'wgpu = { version = "=30.0.1", features = ["metal"] }',
            'winit = "=0.30.13"',
            'pollster = "0.4"',
            'log = "0.4"',
            'env_logger = "0.11"',
        ]
    )
    if kind == "weld-win":
        dependencies.append('raw-window-handle = "=0.6.2"')
    if kind == "weld-linux":
        dependencies.append('raw-window-handle = "=0.6.2"')
    if kind == "weld-mac":
        dependencies.extend(['cef = { version = "151", features = ["accelerated_osr"] }', 'semver = "1"'])
        return header + '''[package.metadata.cef.bundle]
helper_name = "demo-weld-mac-helper"

[[bin]]
name = "demo-weld-mac"
path = "src/main.rs"

[[bin]]
name = "demo-weld-mac-helper"
path = "src/helper.rs"

[[bin]]
name = "bundle-demo-weld-mac"
path = "src/bundle.rs"

[dependencies]
''' + "\n".join(dependencies) + "\n"
    return header + binary + "[dependencies]\n" + "\n".join(dependencies) + "\n"


def copy_required(source: Path, destination: Path, kind: str) -> None:
    demo = source / KIND_TO_DEMO[kind]
    if not (demo / "src").is_dir():
        raise ValueError(f"{demo} has no source directory")
    shutil.copytree(demo / "src", destination / "src")

    if kind == "scry-win":
        scripts = destination / "scripts"
        scripts.mkdir()
        script = source / "scripts" / "test-win.ps1"
        text = script.read_text(encoding="utf-8")
        old = "& cargo build --locked -p demo-win"
        if text.count(old) != 1:
            raise ValueError(f"expected one standalone-rewrite target in {script}")
        (scripts / "test-win.ps1").write_text(text.replace(old, "& cargo build --locked"), encoding="utf-8")
    elif kind == "scry-mac":
        scripts = destination / "scripts"
        scripts.mkdir()
        script = source / "scripts" / "test-mac.sh"
        text = script.read_text(encoding="utf-8")
        old = "cargo build --locked -q -p demo-mac"
        if text.count(old) != 1:
            raise ValueError(f"expected one standalone-rewrite target in {script}")
        staged = scripts / "test-mac.sh"
        staged.write_text(text.replace(old, "cargo build --locked -q"), encoding="utf-8")
        staged.chmod(staged.stat().st_mode | 0o111)
    elif kind == "scry-wpe":
        tests = destination / "tests"
        tests.mkdir()
        for test_name in ("wpe_input.rs", "wpe_to_vulkan_roundtrip.rs"):
            shutil.copy2(source / "scrying" / "tests" / test_name, tests / test_name)
    elif kind.startswith("weld-"):
        shutil.copytree(source / "testing", destination / "testing")
        scripts = destination / "scripts"
        scripts.mkdir()
        for script_name in ("parity-battery.sh", "run-weld-mac.sh"):
            origin = source / "scripts" / script_name
            staged = scripts / script_name
            if script_name == "parity-battery.sh":
                text = origin.read_text(encoding="utf-8")
                marker = "\necho\nprintf 'battery: %s pass, %s live, %s skip, %s fail\\n'"
                cookie_case = '''
# CEF advertises cookie read/write/delete as supported. The release proof must
# observe the asynchronous read returning the cookie it wrote, not merely that
# `set_cookie` returned success.
run_case cookie 'weld_probe=(w5|parity)' WELD_COOKIE_URL=https://example.com/
'''
                if text.count(marker) != 1:
                    raise ValueError(f"expected one cookie-case insertion point in {origin}")
                staged.write_text(text.replace(marker, cookie_case + marker), encoding="utf-8")
            else:
                shutil.copy2(origin, staged)
            staged.chmod(staged.stat().st_mode | 0o111)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kind", choices=sorted(KIND_TO_DEMO))
    parser.add_argument("--source", type=Path, required=True, help="release-tag sibling checkout")
    parser.add_argument("--destination", type=Path, required=True, help="new standalone consumer directory")
    parser.add_argument("--grafting-version", default="0.6.0")
    parser.add_argument("--scrying-version", default="0.7.0")
    parser.add_argument("--welding-version", default="0.14.0")
    args = parser.parse_args()

    source = args.source.resolve()
    destination = args.destination.resolve()
    if not source.is_dir():
        parser.error(f"source checkout does not exist: {source}")
    if destination.exists():
        parser.error(f"destination must not exist: {destination}")

    destination.mkdir(parents=True)
    try:
        copy_required(source, destination, args.kind)
        (destination / "Cargo.toml").write_text(
            manifest_for(args.kind, args.grafting_version, args.scrying_version, args.welding_version),
            encoding="utf-8",
        )
    except Exception:
        shutil.rmtree(destination, ignore_errors=True)
        raise

    print(f"staged {args.kind} registry consumer: {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
