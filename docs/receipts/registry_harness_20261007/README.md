# Registry harness preparation, 2026-10-07

The workflow copies Graft's immutable `grafting-v0.6.0` fixture source
(`816f3e7857afee863200e1c25b300c43b1532aae`) separately from the current
workflow/helper checkout. This keeps the released Graft/Servo 0.5 scaffold
separate from unreleased Servo 0.7 adapter changes. Each staged consumer
records its actual fixture revision, workflow revision, helper revision, and
helper SHA-256 in `staging-source.json`; the verifier's older
`graft_source_sha` field remains the workflow revision.

Standalone manifests establish their own workspace. WPE test declarations
retain custom-main execution and required WPE features. The native workflow
captures stderr, requires both explicit native success markers, and rejects
SKIP output. Windows uses the three approved stable targets, BelowNormal
priority, one build job, and checks for live compiler/demo owners before each
build or native step. These owner checks observe a point in time; manual
host users must still coordinate with the workflow's hardware lock.

[verify.py](verify.py) checks generated manifests, actual immutable Graft
manifest rewrites, source provenance, and workflow guards without creating
fixture directories or running Cargo/apps. It also rejects missing/duplicate
rewrite targets and unreleased Graft source as negative controls. Python 3.11+
and PyYAML are required. Run it with the interpreter where PyYAML is installed.

[verification-result.json](verification-result.json) binds the passing eight
checks and source hashes to the [raw log](verification-stderr.log). The first
[interpreter control](verification-interpreter-control-result.json) failed
before checks because the child Python lacked PyYAML; its raw output is retained.

This evidence qualifies the harness preparation only. No Cargo/native gate,
registry publication, actual staged consumer, or hosted runner availability
is claimed. Dispatch after publication with exact selected registry versions
and immutable Scry/Weld release-source revisions. The old Graft tag workflow
itself predates important host-lock and harness fixes and is not the new wrapper.
