# Registry Servo fixture dependency correction, 2026-10-07

Run [37631036166](https://github.com/merely-made/wgpu-graft/actions/runs/37631036166)
resolved the released trio correctly, but its copied Servo 0.5 proof fixture
failed to compile RSA before any native battery. M4, Intel, and RADV used the
same Graft lock. Each reported eight E0107 errors: `rsa 0.10.0-rc.18` expects
the lifetime-bearing PKCS#1 key structs, while `pkcs1 0.8.0-rc.5` changed them
to a generic type parameter. This is fixture dependency resolution evidence;
it does not establish a browser or graphics defect.

[dependency-evidence.json](dependency-evidence.json) binds all three original
negative logs, actual failed locks, selected registry packages, RSA's Servo
parent graph, and official source manifests/API. Its log hashes refer to
the immutable Scry release-readiness packet owned by the parent lane.

The earlier successful registry proof
[33916390001](https://github.com/merely-made/wgpu-graft/actions/runs/33916390001)
used `rsa 0.10.0-rc.18` with `pkcs1 0.8.0-rc.4`. Its actual
[Graft lock](prior-m4-graft-Cargo.lock) and
[native success log](prior-m4-registry-graft.log) are retained. Both the earlier
and failed normal dependency trees select only `wgpu 29.0.4` for this fixture.
The earlier proof used Scry 0.7.1; it is compatibility evidence for the
RSA/PKCS#1 pairing, not current Scry 0.7.2 native acceptance.

The stager now adds exact registry `pkcs1 = 0.8.0-rc.4`, with default features
disabled, only to the copied Graft demo manifest. Existing RSA features still
supply alloc/pem. The constraint matches the earlier passing lock and the
registry [RSA dependency requirement](https://crates.io/api/v1/crates/rsa/0.10.0-rc.18/dependencies).
All four workflow jobs share this staging path. No released supplier package,
public dependency requirement, Servo source, triplet version, selected wgpu
row, native assertion, or timeout changes. The existing verifier still generates
and records each fresh lock, then requires locked metadata/tree and registry
sources. This freezes the incompatible prerelease edge; it does not freeze
every unrelated registry dependency.

[verify.py](verify.py) reuses the previous eight pure stager checks and adds
three checks for fixture-only constraint scope, actual passing/failing locks,
and official RSA/PKCS#1 API differences. Copies remain mocked; no consumer
directory, Cargo process, build, or app is launched. Its actual result and
source/artifact hashes are recorded in [verification-result.json](verification-result.json).

Fresh compilation and native acceptance of the corrected graph remain open
until the reviewed new workflow runs. The Windows Scry-only registry consumer
already passed separately; it did not contain this Servo/RSA dependency edge.
