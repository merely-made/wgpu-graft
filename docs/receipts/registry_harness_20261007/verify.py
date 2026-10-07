"""Pure staging checks: no Cargo, native processes, or fixture directories."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
from types import SimpleNamespace
import tomllib
import unittest
from unittest.mock import patch

import yaml

ROOT = Path(__file__).resolve().parents[3]
HELPER = ROOT / "scripts/stage_registry_triplet.py"
FIXTURE = "816f3e7857afee863200e1c25b300c43b1532aae"
namespace = {"__file__": str(HELPER), "__name__": "stager_verification"}
exec(compile(HELPER.read_text(), str(HELPER), "exec"), namespace)


class StagerChecks(unittest.TestCase):
    def test_standalone_manifests_and_exact_registry_versions(self):
        for kind in namespace["KIND_TO_DEMO"]:
            if kind == "graft-servo":
                continue
            with self.subTest(kind=kind):
                manifest = tomllib.loads(namespace["manifest_for"](kind, "0.6.0", "0.7.2", "0.14.1"))
                self.assertEqual(manifest["workspace"], {})
                for name, version in [("grafting", "0.6.0"), ("scrying", "0.7.2"), ("welding", "0.14.1")]:
                    dependency = manifest["dependencies"][name]
                    self.assertEqual(dependency["version"], "=" + version)
                    self.assertNotIn("git", dependency)
                    self.assertNotIn("path", dependency)

    def test_wpe_custom_main_contract(self):
        manifest = tomllib.loads(namespace["manifest_for"]("scry-wpe", "0.6.0", "0.7.2", "0.14.1"))
        self.assertEqual({test["name"] for test in manifest["test"]}, {"wpe_input", "wpe_to_vulkan_roundtrip"})
        for test in manifest["test"]:
            self.assertIs(test["harness"], False)
            self.assertEqual(test["required-features"], ["wpe"])
        self.assertEqual(manifest["features"]["default"], ["wgpu-30"])

    def graft_manifests(self, revision):
        written = {}
        source, destination = Path("fixture"), Path("consumer")

        def read_fixture(path, **_):
            relative = path.relative_to(source).as_posix()
            return subprocess.check_output(["git", "show", f"{revision}:{relative}"], cwd=ROOT, text=True, encoding="utf-8")

        def record_write(path, text, **_):
            written[path.relative_to(destination).as_posix()] = text

        with patch.object(Path, "read_text", read_fixture), patch.object(Path, "write_text", record_write), \
                patch.object(Path, "mkdir"), patch.object(Path, "chmod"), \
                patch.object(Path, "stat", return_value=SimpleNamespace(st_mode=0o644)), \
                patch.object(namespace["shutil"], "copytree"), patch.object(namespace["shutil"], "copy2"):
            namespace["copy_required"](source, destination, "graft-servo", "0.6.0", "0.7.2", "0.14.1")
        return written

    def test_actual_immutable_graft_fixture_rewrites(self):
        written = self.graft_manifests(FIXTURE)
        adapter = tomllib.loads(written["adapter/Cargo.toml"])
        demo = tomllib.loads(written["demo/Cargo.toml"])
        self.assertEqual(adapter["dependencies"]["grafting"]["version"], "=0.6.0")
        self.assertEqual(adapter["dependencies"]["servo"]["rev"], namespace["SERVO_REV"])
        self.assertEqual(demo["dependencies"]["servo"]["rev"], namespace["SERVO_REV"])
        for name, version in [("grafting", "0.6.0"), ("scrying", "0.7.2"), ("welding", "0.14.1")]:
            self.assertEqual(demo["dependencies"][name]["version"], "=" + version)
        self.assertNotIn("path", adapter["dependencies"]["grafting"])
        self.assertEqual(demo["dependencies"]["scrying"]["features"], ["wgpu-29"])
        self.assertEqual(demo["dependencies"]["welding"]["features"], ["wgpu-29"])

    def test_unreleased_graft_source_is_not_silently_accepted(self):
        with self.assertRaisesRegex(ValueError, "expected one registry rewrite target"):
            self.graft_manifests("HEAD")

    def test_rewrite_missing_and_duplicate_negative_controls(self):
        for text in ["missing", "old old"]:
            with self.assertRaises(ValueError):
                namespace["replace_once"](text, "old", "new", Path("fixture"))

    def test_distinct_fixture_helper_and_workflow_provenance(self):
        written = {}
        with patch.object(namespace["subprocess"], "check_output", side_effect=[FIXTURE + "\n", "helper-revision\n"]), \
                patch.dict(os.environ, {"GITHUB_SHA": "wrapper-revision"}), \
                patch.object(Path, "write_text", lambda path, text, **_: written.update({str(path): text})):
            namespace["record_staging_source"](Path("fixture"), Path("consumer"), "graft-servo")
        facts = json.loads(next(iter(written.values())))
        self.assertEqual(facts["fixture_source_sha"], FIXTURE)
        self.assertEqual(facts["helper_source_sha"], "helper-revision")
        self.assertEqual(facts["workflow_source_sha"], "wrapper-revision")
        self.assertEqual(facts["helper_sha256"], hashlib.sha256(HELPER.read_bytes()).hexdigest())

    def test_workflow_immutable_fixture_and_wpe_acceptance_guards(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/registry-only-triplet.yml").read_text())
        self.assertEqual(len(workflow["jobs"]), 4)
        for job in workflow["jobs"].values():
            checkouts = [step.get("with", {}) for step in job["steps"] if step.get("uses") == "actions/checkout@v4"]
            fixtures = [checkout for checkout in checkouts if checkout.get("path") == ".registry-proof-source/graft"]
            self.assertEqual(len(fixtures), 1)
            self.assertEqual(fixtures[0]["ref"], FIXTURE)
            stage = next(step["run"] for step in job["steps"] if step.get("name") == "Stage and verify registry-only consumers")
            self.assertIn("--kind graft-servo --source .registry-proof-source/graft", stage)
        wpe = next(step["run"] for step in workflow["jobs"]["radv-vulkan"]["steps"] if step.get("name", "").startswith("Run WPE"))
        self.assertEqual(wpe.count("--nocapture 2>&1 | tee"), 2)
        for name in ["wpe_input", "wpe_to_vulkan_roundtrip"]:
            self.assertIn(f'grep -Fx "NATIVE WPE GATE PASS: {name}"', wpe)
        self.assertIn("! grep -Eq '^SKIP:'", wpe)

    def test_windows_stable_targets_priority_and_failure_guards(self):
        job = yaml.safe_load((ROOT / ".github/workflows/registry-only-triplet.yml").read_text())["jobs"]["nvidia-dx12"]
        self.assertEqual(job["env"]["CARGO_BUILD_JOBS"], "1")
        scripts = [step["run"] for step in job["steps"] if step.get("shell") == "pwsh" and "\n" in step.get("run", "")]
        self.assertEqual(len(scripts), 5)
        for script in scripts:
            self.assertIn("PriorityClass = 'BelowNormal'", script)
            self.assertIn("Shared build/native slot is occupied", script)
        combined = "\n".join(scripts)
        for kind in ["graft", "scry", "weld"]:
            self.assertIn(f"'C:\\t\\cargo-targets\\wgpu-{kind}'", combined)
        self.assertNotIn("registry-triplet-target", combined)
        stage = next(step["run"] for step in job["steps"] if step.get("name") == "Stage and verify registry-only consumers")
        lines = stage.splitlines()
        for index, line in enumerate(lines):
            if line.strip().startswith("python scripts/"):
                self.assertEqual(lines[index + 1].strip(), "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }")


if __name__ == "__main__":
    unittest.main(verbosity=2)
