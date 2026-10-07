"""Pure scope, missing-field and syntax checks; no macOS observer execution."""

import ast
import importlib.util
from pathlib import Path
import subprocess
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github/workflows/scry-registry-intel-diagnostic.yml"
HELPER = ROOT / "scripts/observe_scry_mac_session.py"
spec = importlib.util.spec_from_file_location("observer", HELPER)
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class ObserverChecks(unittest.TestCase):
    def test_missing_wrong_type_and_explicit_false(self):
        self.assertEqual(observer.session_fields({}), {"session_uid": None, "on_console": None,
                                                     "login_done": None, "locked": None})
        values = {"kCGSSessionUserIDKey": "name", "kCGSSessionOnConsoleKey": 1,
                  "kCGSessionLoginDoneKey": False, "CGSSessionScreenIsLocked": None,
                  "user_name": "never emitted"}
        fields = observer.session_fields(values)
        self.assertIsNone(fields["session_uid"])
        self.assertIsNone(fields["on_console"])
        self.assertIs(fields["login_done"], False)
        self.assertIsNone(fields["locked"])
        self.assertNotIn("user_name", fields)

    def test_exact_own_bundle_and_executable_both_required(self):
        bundle = Path("/approved/scry.app")
        path = str(bundle / "Contents/MacOS/demo-mac")
        self.assertTrue(observer.owned_paths(path, str(bundle), bundle))
        self.assertFalse(observer.owned_paths(path + "-other", str(bundle), bundle))
        self.assertFalse(observer.owned_paths(path, "/foreign/scry.app", bundle))
        self.assertFalse(observer.owned_paths(None, None, bundle))

    def test_existing_batteries_and_identity_unchanged(self):
        before = yaml.safe_load(subprocess.check_output(["git", "show",
            "0e491151d6943d04959fe0ad31477602806ed9df:.github/workflows/scry-registry-intel-diagnostic.yml"], cwd=ROOT))
        after = yaml.safe_load(WORKFLOW.read_text())
        trigger = after.get("on", after.get(True))
        self.assertIs(trigger["workflow_dispatch"]["inputs"]["observe_session"]["default"], False)
        self.assertEqual(before["env"], after["env"])
        steps = after["jobs"]["intel-scry-only"]["steps"]
        additions = [s for s in steps if s.get("id") == "session_observer" or s.get("name", "").startswith("Stop and retain passive")]
        self.assertEqual(len(additions), 2)
        self.assertEqual(before["jobs"]["intel-scry-only"]["steps"], [s for s in steps if s not in additions])
        start, stop = additions
        self.assertTrue(start["continue-on-error"])
        self.assertLess(steps.index(start), next(i for i,s in enumerate(steps) if s.get("id") == "ordinary_battery"))
        self.assertIn("always()", stop["if"])
        self.assertNotIn("!cancelled()", stop["if"])
        self.assertNotIn("continue-on-error", stop)
        self.assertIn("native batteries", WORKFLOW.read_text())
        self.assertIn("session-observer.stop", stop["run"])
        self.assertIn("sample_count", stop["run"])
        self.assertIn("nice -n 10 python3 -B", start["run"])

    def test_syntax_bounds_and_redacted_projection(self):
        source = HELPER.read_text()
        ast.parse(source)
        for forbidden in ["activateWithOptions", "unhide", "CGWindowListCreateImage", "localizedName", "kCGWindowName", "kCGWindowOwnerName", "os.environ", "os.kill", "subprocess.Popen"]:
            self.assertNotIn(forbidden, source)
        self.assertIn("CFRunLoopRunInMode(self.mode, 0.01, False)", source)
        self.assertIn("count < 600", source)
        self.assertIn("time.monotonic() - started < 1800", source)
        self.assertIn("timeout=1", source)
        self.assertLess(source.index("if pid not in own_pids:"), source.index("own_windows.append"))
        workflow = yaml.safe_load(WORKFLOW.read_text())
        for step in workflow["jobs"]["intel-scry-only"]["steps"]:
            code = step.get("run")
            if not code:
                continue
            result = subprocess.run(["C:/Program Files/Git/bin/bash.exe", "-n"], input=code.encode(), capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            if "<<'PY'" in code:
                compile(code.split("<<'PY'\n",1)[1].rsplit("\nPY",1)[0], "embedded_python", "exec")


if __name__ == "__main__":
    unittest.main(verbosity=2)
