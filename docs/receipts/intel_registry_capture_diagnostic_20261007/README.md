# Scry-only Intel registry capture diagnostic, 2026-10-07

The four-host registry run's Intel job failed the ordinary capture minimum:
three acquired/Complete frames, 1741 Idle samples, visible/nonminimized host,
and no host focus. Its resize receipt passed with twelve acquired frames.
The earlier source-native Intel attempt passed with host focus, and its
separate activity fixture showed visible DOM state and advancing animation.
This correlation does not establish a focus, session, driver, or library cause.
The original failure remains retained in the parent Scry release packet.

[source-and-control-review.json](source-and-control-review.json) binds the raw
logs and exact source comparison. The Mac fixture and script have no changes
between source-native revision 4771072 and released Scry revision 4d8d1d3.
The source-native lock used objc2 0.6.4 while registry staging requests 0.6.3;
that graph distinction is recorded without attributing the failure to it.
Direct SSH to Intel was refused; this is a transport constraint, not evidence
about the user's reported unlocked session.

The dedicated [workflow](../../../.github/workflows/scry-registry-intel-diagnostic.yml)
runs only the Intel Scry registry consumer. It checks out Scry
`4d8d1d3f8dd450b712960c90b74263a45b46e35b` and immutable helpers
`e8ffee2d6b680154cae645d54a340116f08acb5d`, pins registry trio versions
0.6.0/0.7.2/0.14.1, and records checkout/helper, package VCS/graph/lock,
LaunchServices/signing, and raw/signed executable identities. It uses the
existing host lock, authorized target and bundle path, and awake hold.
Build and battery-shell launches use nice 10; the battery's cached rebuild
inherits that priority. Test deadlines are unchanged.

The unchanged ten-mode battery runs first. A separate `CAPTURE_ACTIVITY=1`
two-mode diagnosis runs after ordinary success or failure unless cancelled.
This is the shipped control: it adds `--capture-activity` and changes only the
diagnostic page URL to `/capture?activity=1`. There is no shipped
`SCRY_CAPTURE_ACTIVITY_TRACE` environment switch. The original failure remains
a failed workflow step and job; activity success cannot accept it. The
five-frame/30-second capture requirement and 90-second per-mode wall cap remain.
No focus manipulation, forced paint, or supplier source change is included.

[verify.py](verify.py) checks scope, pinned manifests, failure retention, and
Bash/embedded-Python syntax without Cargo, apps, staging, or dispatch.
[verification-result.json](verification-result.json) records its actual result
and source hashes. Root reviews and dispatches the published workflow; these
preparation checks do not claim native acceptance or replace the four-host bar.
The retained verification control failed because its checkout selector also
matched the artifact upload's path; the corrected selector passed all seven
checks. The failed test source, workflow snapshot, and raw result are retained.
