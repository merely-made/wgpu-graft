"""Fixture-only RSA compatibility checks; no Cargo or actual staging."""

import importlib.util
import json
from pathlib import Path
import tomllib
import unittest

HERE = Path(__file__).resolve().parent
previous = HERE.parent / "registry_harness_20261007/verify.py"
spec = importlib.util.spec_from_file_location("previous_stager_checks", previous)
verification = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verification)


class RsaChecks(verification.StagerChecks):
    def test_constraint_is_only_in_the_copied_servo_fixture(self):
        manifest = tomllib.loads(self.graft_manifests(verification.FIXTURE)["demo/Cargo.toml"])
        self.assertEqual(manifest["dependencies"]["pkcs1"], {
            "version": "=0.8.0-rc.4", "default-features": False,
        })
        for kind in verification.namespace["KIND_TO_DEMO"]:
            if kind != "graft-servo":
                manifest = tomllib.loads(verification.namespace["manifest_for"](kind, "0.6.0", "0.7.2", "0.14.1"))
                self.assertNotIn("pkcs1", manifest["dependencies"])

    def test_prior_success_and_three_failed_hosts_bind_actual_versions(self):
        proof = json.loads((HERE / "dependency-evidence.json").read_text())
        prior = proof["artifacts"][0]
        self.assertEqual(prior["run"], 33916390001)
        versions = {p["name"]: p["version"] for p in prior["selected_packages"]}
        self.assertEqual(versions["rsa"], "0.10.0-rc.18")
        self.assertEqual(versions["pkcs1"], "0.8.0-rc.4")
        self.assertIn("GRAFT DEMO SMOKE PASS", (HERE / "prior-m4-registry-graft.log").read_text())
        failures = proof["artifacts"][1:]
        self.assertEqual(len(failures), 3)
        self.assertEqual(len({p["lock_sha256"] for p in failures}), 1)
        for failure in failures:
            versions = {p["name"]: p["version"] for p in failure["selected_packages"]}
            self.assertEqual(versions["rsa"], "0.10.0-rc.18")
            self.assertEqual(versions["pkcs1"], "0.8.0-rc.5")
            self.assertEqual(versions["grafting"], "0.6.0")
            self.assertEqual(versions["scrying"], "0.7.2")
            self.assertEqual(versions["welding"], "0.14.1")

    def test_official_rsa_requirement_and_pkcs1_api_break(self):
        rsa = tomllib.loads((HERE / "rsa-0.10.0-rc.18-Cargo.toml").read_text())
        self.assertEqual(rsa["dependencies"]["pkcs1"]["version"], "0.8.0-rc.4")
        api = json.loads((HERE / "rsa-rc18-dependencies.json").read_text())
        self.assertEqual(next(p["req"] for p in api["dependencies"] if p["crate_id"] == "pkcs1"), "^0.8.0-rc.4")
        for file, struct in [("private_key", "RsaPrivateKey"), ("public_key", "RsaPublicKey")]:
            old = (HERE / f"pkcs1-0.8.0-rc.4-src-{file}.rs").read_text()
            new = (HERE / f"pkcs1-0.8.0-rc.5-src-{file}.rs").read_text()
            self.assertIn(f"pub struct {struct}<'a>", old)
            self.assertIn(f"pub struct {struct}<U>", new)


if __name__ == "__main__":
    unittest.main(verbosity=2)
