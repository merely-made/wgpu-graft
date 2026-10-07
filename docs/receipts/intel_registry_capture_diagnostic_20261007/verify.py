"""Pure scope/syntax verification; no host apps, Cargo, or staging."""

import hashlib
from pathlib import Path
import subprocess
import tomllib
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github/workflows/scry-registry-intel-diagnostic.yml"
HELPER = "e8ffee2d6b680154cae645d54a340116f08acb5d"
SCRY = "4d8d1d3f8dd450b712960c90b74263a45b46e35b"


class DiagnosticChecks(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.safe_load(WORKFLOW.read_text())
        self.job = self.workflow["jobs"]["intel-scry-only"]
        self.steps = self.job["steps"]

    def test_only_intel_with_shared_host_lock(self):
        self.assertEqual(list(self.workflow["jobs"]), ["intel-scry-only"])
        self.assertEqual(self.job["runs-on"], ["self-hosted", "wgpu-hardware", "metal", "macos-x64", "intel-mac", "headed"])
        self.assertTrue(any(s.get("uses") == "./.github/actions/host-hardware-lock" for s in self.steps))
        self.assertFalse(self.workflow["concurrency"]["cancel-in-progress"])

    def test_exact_fixture_helper_and_registry_versions(self):
        refs = {s["with"]["path"]: s["with"]["ref"] for s in self.steps
                if s.get("uses") == "actions/checkout@v4" and "path" in s.get("with", {})}
        self.assertEqual(refs, {".registry-proof-helper": HELPER, ".registry-proof-source/scry": SCRY})
        env = self.workflow["env"]
        for key, value in [("GRAFTING_VERSION", "0.6.0"), ("SCRYING_VERSION", "0.7.2"), ("WELDING_VERSION", "0.14.1")]:
            self.assertEqual(env[key], value)
        stage = next(s["run"] for s in self.steps if s.get("name", "").startswith("Stage and verify"))
        self.assertEqual(stage.count("--kind "), 1)
        self.assertIn("--kind scry-mac", stage)
        self.assertIn("verify_registry_triplet.py", stage)
        self.assertNotIn("--allow-servo-git", stage)
        self.assertNotIn("graft-servo", stage)
        self.assertNotIn("weld-mac", stage)

    def test_stable_target_and_unchanged_caps(self):
        env = self.workflow["env"]
        self.assertEqual(env["CARGO_TARGET_DIR"], "/Users/markik/Code/targets/wgpu-scry-hardware")
        self.assertEqual(env["SCRY_MAC_APP"], env["CARGO_TARGET_DIR"] + "/debug/scry-demo-mac-hardware.app")
        self.assertEqual(env["TIMEOUT"], "90")
        self.assertEqual(env["CAPTURE"], "1")
        self.assertEqual(env["VISIBLE"], "1")

    def test_original_failure_cannot_become_diagnostic_success(self):
        ordinary = next(s for s in self.steps if s.get("id") == "ordinary_battery")
        diagnostic = next(s for s in self.steps if "if" in s and "ordinary_battery" in s["if"])
        self.assertEqual(ordinary["env"]["CAPTURE_ACTIVITY"], "0")
        self.assertEqual(diagnostic["env"]["CAPTURE_ACTIVITY"], "1")
        self.assertNotIn("continue-on-error", ordinary)
        self.assertNotIn("continue-on-error", diagnostic)
        self.assertEqual(diagnostic["if"], "${{ always() && !cancelled() && steps.ordinary_battery.outcome != 'skipped' }}")
        for step in [ordinary, diagnostic]:
            code = step["run"]
            self.assertIn('|| status=$?', code)
            self.assertIn('exit "$status"', code)
            self.assertIn("set -o pipefail", code)
            self.assertIn("caffeinate -u -t 5", code)
            self.assertIn("caffeinate -dims bash", code)
            self.assertIn("nice -n 10 caffeinate -dims bash", code)
            self.assertIn("codesign -d -r-", code)
            self.assertIn("cmp ", code)
            self.assertIn("$SCRY_MAC_APP/Contents/MacOS/demo-mac", code)
        self.assertLess(self.steps.index(ordinary), self.steps.index(diagnostic))

    def test_no_focus_or_published_source_manipulation(self):
        code = "\n".join(s.get("run", "") for s in self.steps)
        for forbidden in ["focus_window", "activateIgnoringOtherApps", "tell application", "cargo publish", "cargo update", "git reset", "SCRY_CAPTURE_ACTIVITY_TRACE"]:
            self.assertNotIn(forbidden, code)
        self.assertIn("Shared compiler/native slot is occupied", code)
        self.assertIn("registry Scry package does not bind", code)
        self.assertIn("nice -n 10 cargo build --locked -j 1", code)
        self.assertIn("fixture", code)

    def test_immutable_helper_produces_registry_manifest_in_memory(self):
        source = subprocess.check_output(["git", "show", f"{HELPER}:scripts/stage_registry_triplet.py"], cwd=ROOT)
        namespace = {"__name__": "diagnostic_verification", "__file__": str(ROOT / "scripts/stage_registry_triplet.py")}
        exec(compile(source, "immutable_stager", "exec"), namespace)
        manifest = tomllib.loads(namespace["manifest_for"]("scry-mac", "0.6.0", "0.7.2", "0.14.1"))
        self.assertEqual(manifest["workspace"], {})
        for name, version in [("grafting", "0.6.0"), ("scrying", "0.7.2"), ("welding", "0.14.1")]:
            self.assertEqual(manifest["dependencies"][name]["version"], "=" + version)
            self.assertNotIn("path", manifest["dependencies"][name])
            self.assertNotIn("git", manifest["dependencies"][name])

    def test_bash_and_embedded_python_syntax(self):
        for step in self.steps:
            code = step.get("run")
            if not code:
                continue
            result = subprocess.run(["C:/Program Files/Git/bin/bash.exe", "-n"], input=code.encode(), capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            if "<<'PY'" in code:
                python = code.split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
                compile(python, "embedded_registry_identity", "exec")


if __name__ == "__main__":
    unittest.main(verbosity=2)
