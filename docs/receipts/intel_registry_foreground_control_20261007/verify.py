"""Pure counted-rewrite and scope checks; no Cargo or app execution."""

import importlib.util
from pathlib import Path
import subprocess
import unittest

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
WORKFLOW = ROOT / ".github/workflows/scry-registry-intel-diagnostic.yml"
previous = HERE.parent / "intel_registry_capture_diagnostic_20261007/verify.py"
spec = importlib.util.spec_from_file_location("baseline_checks", previous)
baseline = importlib.util.module_from_spec(spec)
spec.loader.exec_module(baseline)


def generator():
    workflow = yaml.safe_load(WORKFLOW.read_text())
    step = next(s for s in workflow["jobs"]["intel-scry-only"]["steps"] if s.get("id") == "foreground_script")
    code = step["run"].split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
    namespace = {"__name__": "pure_rewrite_checks"}
    exec(compile(code, "foreground_generator", "exec"), namespace)
    return workflow, namespace["foreground_script"]


def staged_original():
    source = subprocess.check_output(["git", "show", "4d8d1d3f8dd450b712960c90b74263a45b46e35b:scripts/test-mac.sh"],
                                     cwd="C:/Users/mark_/Code/repos/wgpu-scry", text=True)
    old = "cargo build --locked -q -p demo-mac"
    assert source.count(old) == 1
    return source.replace(old, "cargo build --locked -q")


class ForegroundChecks(baseline.DiagnosticChecks):
    def test_existing_baseline_steps_are_identical(self):
        before = yaml.safe_load(subprocess.check_output(
            ["git", "show", "0201765fdbfa136ef034826c23e4f97331623890:.github/workflows/scry-registry-intel-diagnostic.yml"], cwd=ROOT))
        after, _ = generator()
        self.assertEqual(before["env"], after["env"])
        self.assertEqual(before["jobs"]["intel-scry-only"]["steps"], [
            step for step in after["jobs"]["intel-scry-only"]["steps"]
            if step.get("id") != "foreground_script"
            and not step.get("name", "").startswith("Run separately foreground-controlled")])

    def test_foreground_default_off_and_baselines_first(self):
        workflow, _ = generator()
        trigger = workflow.get("on", workflow.get(True))
        option = trigger["workflow_dispatch"]["inputs"]["foreground_control"]
        self.assertEqual(option["type"], "boolean")
        self.assertIs(option["default"], False)
        steps = workflow["jobs"]["intel-scry-only"]["steps"]
        ordinary = next(s for s in steps if s.get("id") == "ordinary_battery")
        activity = next(s for s in steps if s.get("name", "").startswith("Diagnose activity"))
        generate = next(s for s in steps if s.get("id") == "foreground_script")
        self.assertLess(steps.index(ordinary), steps.index(activity))
        self.assertLess(steps.index(activity), steps.index(generate))
        self.assertIn("inputs.foreground_control", generate["if"])
        self.assertIn("steps.ordinary_battery.outcome != 'skipped'", generate["if"])
        controlled = [s for s in steps if s.get("name", "").startswith("Run separately foreground-controlled")]
        self.assertEqual(len(controlled), 2)
        self.assertEqual([s["env"]["CAPTURE_ACTIVITY"] for s in controlled], ["0", "1"])
        for step in controlled:
            self.assertIn("always() && !cancelled()", step["if"])
            self.assertIn("steps.foreground_script.outcome == 'success'", step["if"])
            self.assertNotIn("continue-on-error", step)
            self.assertIn('exit "$status"', step["run"])
            self.assertIn("nice -n 10 caffeinate -dims bash", step["run"])
            self.assertIn("test-mac-foreground-control.sh", step["run"])
            self.assertIn("cmp ", step["run"])

    def test_actual_script_rewrite_and_bash_syntax(self):
        _, build = generator()
        before = staged_original()
        after = build(before)
        reset = "    foreground_control_attempted=0\n"
        self.assertEqual(after.count(reset), 1)
        self.assertEqual(after.count('open -a "$DEMO_APP"'), 1)
        self.assertIn('if kill -0 "$_pid"', after)
        observation = after.index("        if demo_app_running; then")
        activation = after.index('open -a "$DEMO_APP"')
        following = after.index("        elif [[ $saw_process -eq 1 ]]")
        self.assertLess(observation, activation)
        self.assertLess(activation, following)
        self.assertLess(after.index("foreground_control_attempted=1"), activation)
        # Removing exactly the two inserted blocks reproduces the original.
        start = after.index("            if [[ \"$foreground_control_attempted\"")
        end = after.index("        elif [[ $saw_process -eq 1 ]]", start)
        restored = (after[:start] + after[end:]).replace(reset, "", 1)
        self.assertEqual(restored, before)
        result = subprocess.run(["C:/Program Files/Git/bin/bash.exe", "-n"], input=after.encode(), capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_missing_or_duplicate_rewrite_boundaries_fail_closed(self):
        _, build = generator()
        before = staged_original()
        for marker in ["    saw_process=0\n", "        if demo_app_running; then\n            saw_process=1\n"]:
            for altered in [before.replace(marker, "", 1), before + marker]:
                with self.assertRaisesRegex(ValueError, "expected exactly one"):
                    build(altered)


if __name__ == "__main__":
    unittest.main(verbosity=2)
