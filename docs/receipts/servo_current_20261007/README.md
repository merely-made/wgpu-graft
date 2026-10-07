# Current Servo migration, 2026-10-07

**Status:** the immutable Servo 0.7.0 migration passes eleven Windows typed
checks and the Windows DX12/wgpu 30 donor GPU smoke. This is a supplier source
checkpoint. Turnstone's new consumer qualification, accessibility acceptance,
reopened-view reliability, wider platform runtime checks, and registry release
gates remain separate.

The [canonical release plan](../../../design_docs/2026-09-03_wgpu_triplet_release_plan.md)
owns the remaining gates. The [earlier presentation/ordering receipt](../servo_present_hook_20261006/README.md)
retains the older source, failures, and six same-binary synchronization controls.

## Source and dependency family

Official [release metadata](https://github.com/servo/servo/releases/tag/v0.7.0)
selects latest stable v0.7.0, released October 5. The immutable revision is
`aac43a3f31a259f04a574f5ec4e959c943ad7cc7`. The raw [release response](upstream-release.json)
and [tag response](upstream-tag.json) are preserved. All ten Servo declarations
across nine adapter/demo manifests use that same exact revision. The root and
isolated Iced locks each contain one Servo source family, Surfman 0.14.0,
mozangle 0.7.1, and IPC channel 0.23.0. AccessKit remains 0.24.1 in Servo's
family; independent framework families are not an accessibility integration
claim. See the [manifest audit](migration-manifest-audit.json),
[root resolved family](root-lock-family.json), and
[Iced resolved family](iced-lock-family.json).

Surfman 0.14 makes `chains` unconditional, so its obsolete feature request is
removed. Windows `sm-angle-default`, the donor's upstream `no-wgl` selection,
and matching ANGLE DLL construction remain explicit. No framework or host wgpu
row is silently downgraded. The adapter implementation and core Rust rendering
code are unchanged. Six demo files map Servo's renamed buttons from
Left/Middle/Right to Primary/Auxiliary/Secondary at 22 sites, preserving each
framework's button values. The [mapping record](mouse-button-api-migration.json)
and [pre-correction source archive](pre-api-fix-sources.zip) preserve the actual
API correction. Three donor comments clarify the GPU normalization copy.

## Typed checks

The [matrix result](typed-matrix-results.json) records the exact commands,
environments, log hashes, and source verification for all eleven passing rows:

| Row | Result |
| --- | --- |
| Graft wgpu 28 + Surfman | PASS |
| Graft wgpu 29 + Surfman | PASS |
| Graft wgpu 30 + Surfman | PASS |
| Default winit donor, wgpu 29 | PASS |
| Xilem | PASS |
| GPUI | PASS |
| Bevy | PASS |
| Blitz | PASS |
| egui | PASS |
| Slint | PASS |
| Isolated Iced workspace, wgpu 28 | PASS |

The native donor build separately compiles the wgpu 30 + Servo row. Every final
row uses Rust 1.97.1, `--locked`, `RUSTFLAGS=-D warnings`, desktop LLVM, and the
stable `C:/t/cargo-targets/wgpu-graft` target at BelowNormal priority. Core-only
checks use one job. Servo graphs use two jobs, as required by the maintained
WebRender jobserver prerequisite. Demos are checked individually to preserve
their independent feature graphs. All 84 frozen build inputs remained
unchanged throughout these checks and the native build.

The first one-job attempt was stopped after recording the WebRender worker
wait; see the [diagnostic](donor30-j1-wait-diagnostic.json) and its raw log. The
first two-job attempt exposed four actual donor mouse-button API errors. Its
failed result and log remain beside the successful corrected check. A nonfatal
upstream tidy negative-fixture manifest diagnostic appears during Cargo source
discovery; the successful invocations still exit zero. No upstream cache or
source suppression was applied.

## Windows native GPU result

The [native build result](donor30-current-servo-native-build-result.json),
[Cargo artifact stream](donor30-current-servo-native-build.jsonl), and
[artifact binding](native30-build-binding.json) identify the actual completed
native unit. Its ANGLE OUT_DIR is
`mozangle-315def473559b043/out`, distinct from typed-check outputs. The
[native source archive](native30-source-inputs.zip) preserves all 84 raw input
files; its manifest is [donor30-native-inputs.json](donor30-native-inputs.json).
Normal Git line-ending filters are audited separately in the
[Git source comparison](git-source-equivalence.json): 72 inputs are byte-exact
and 12 differ only through standard CRLF-to-LF filtering. The
[final audit](audit-result.json) and [artifact manifest](artifact-manifest.json)
record the finished result/log/source checks and receipt hashes.

| Artifact | SHA-256 |
| --- | --- |
| demo-servo-winit.exe | `870376ebaee07f808cddbf31dfc8ccc5c439e4df862190ef2c079ee69f1b5597` |
| libEGL.dll | `72f1b00f60cd38a7a0f377eebd34e35aa630ee20a4184aca932e621f89439d69` |
| libGLESv2.dll | `e3178db217a98504b50e6d32822b6048f1b3f584305886299e1e7d0da8043c07` |

The guarded [runner](run-winit-smoke.ps1) stages those exact DLLs, requests DX12,
refuses receipt overwrites, and captures actual loaded module paths/hashes and
file/product versions. The [runtime provenance](current-servo-winit-wgpu30-smoke-provenance.json)
records native exit zero without timeout and both matching loaded modules.
Their version resources are absent, recorded as null. The PE import logs
remain beside the binding.

The [stdout](current-servo-winit-wgpu30-smoke.stdout.log) verifies initial
`[23,97,181,255]` at 1024x600, then a forwarded primary click and actual resize
produce `[221,79,54,255]` at 960x640 on the GPU-import path (six observed frames).
The smoke rejects CPU fallback. Regular presentation performs no CPU pixel
readback; this bounded verifier reads a texel to check the imported frame.
`DiagnosticGpuSync::Existing` remains the default, and PreserveBuffer::No is
retained with the pinned source's full-context clear and buffer-age-zero render.
The Windows donor ends through `process::exit`, so this proves the bounded pixel
gate rather than full Servo process-root shutdown.

## Limits and publication

The preceding Existing control passed only one of two reopened-view pixel
reviews. The newer one-view donor smoke does not prove that intermittent defect
fixed, and does not justify changing synchronization defaults. Servo 0.7's
public accessibility action forwarder does not by itself qualify host tree
grafting, action advertisement, supported action behavior, or assistive
technology acceptance. Linux/macOS native gates and fresh remote CI for this
source remain open.

Read-only formatting checks still fail: 138 root-workspace paths and the Iced
demo contain existing debt. The [format results](format-results.json) preserve
both logs. The [changed-source comparison](changed-source-format-equivalence.json)
proves formatted HEAD plus only the reviewed button/comment substitutions equals
formatted current source for all six changed Rust files. No broad formatter
changes were applied. Git whitespace checking passes.

The qualified source commit and its raw native archive are retained separately
from the subsequent documentation-only exact installation recipe. That recipe
points to the qualified source revision, and does not claim its changed compiled
documentation bytes received another native run. No crate version, tag, or
registry publication is part of this checkpoint.

## Opt-in resize and import observation, 2026-10-07

`GRAFT_SERVO_RESIZE_TRACE=1` enables supplier diagnostics, sampled once per
crate on first use. They record changed resize/swap results, actual Surfman
surface IDs and extents, logical dimensions, import/handoff dimensions and
acquisition generations. The GL hook and DX12 blit also record viewport,
scissor and framebuffer bindings. Acquisition generation counts calls; it
does not prove a new WebRender content epoch. The trace performs no pixel
readback or additional GPU completion wait. Default synchronization,
PreserveBuffer::No and immutable Servo AAC remain unchanged.

This supports a separate Turnstone control whose native DOM reported width
509 after resize while the captured page still showed its earlier 252-wide
layout. The GL blit retains incoming scissor and changes framebuffer bindings;
that is a source-qualified state-custody concern, not an established cause or
fix. Native comparison of the instrumented supplier remains pending.

The [typed result](resize-trace-typed-result.json) records one Windows
`cargo +1.97.1 check --locked -p demo-servo-winit --no-default-features
--features wgpu-30,servo --target-dir C:/t/cargo-targets/wgpu-graft -j1`
with `-D warnings`, BelowNormal priority, exit zero and all 40 inputs unchanged.
It checks the changed Graft and adapter sources through the cached donor graph.
The [raw log](resize-trace-typed.log) retains Servo tidy fixture discovery's
malformed-manifest diagnostic and package-cache waits before the successful
compiler result. No donor application ran. The [input index](resize-trace-inputs.json),
[raw source ZIP](resize-trace-inputs.zip), [reviewed diff](resize-trace-source.diff)
and [evidence index](resize-trace-evidence.json) bind this checkpoint separately
from the earlier A native qualification. Formatting and wider platform gates
retain their prior scope; this typed row does not qualify a pixel fix or AT.

### Shared resource descriptor observation (2026-10-07)

The same opt-in trace now queries `ID3D12Resource::GetDesc` immediately after
`OpenSharedHandle`, recording actual dimensions, flags, layout, format, mip
count and sample count. This adds observation only. The [typed result](resource-desc-trace-typed-result.json)
passes the same Windows wgpu-30/Servo command at BelowNormal `-j1` in 13.58s,
with all forty [inputs](resource-desc-trace-inputs.json) unchanged. The
[raw source archive](resource-desc-trace-inputs.zip), [diff](resource-desc-trace-source.diff),
[log](resource-desc-trace-typed.log) and [evidence index](resource-desc-trace-evidence.json)
bind this checkpoint. No native descriptor value has been measured here.

The resource-state gate precedes any fence implementation. Microsoft documents
[automatic COMMON decay](https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-resource-barriers-to-synchronize-resource-states-in-direct3d-12)
for simultaneous-access textures after GPU command-list execution, including
explicit transitions. The first wgpu 30 read explicitly transitions the shared
alias from UNINITIALIZED/COMMON to RESOURCE. Subsequent canonical normalization
uses only shader reads of this private alias, creating a fresh output texture;
COMMON can implicitly promote to those reads. Actual
`D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS` must be observed before relying
on that decay. Without it, a coherent tracked COMMON handback is required;
queue fences alone do not establish the resource state.

The existing Windows features already expose `ID3D11Device5::OpenSharedFence`,
`ID3D11DeviceContext4::Signal/Wait` and D3D12 shared fences. The producer device
comes from Surfman's ANGLE device; mozangle 0.7.1's `Renderer11::flush` uses its
immediate context, also obtainable with `GetImmediateContext`. The host queue
is obtained through `Queue::as_hal::<Dx12>().as_raw()`, as in `sync_dx12.rs`.
Those are implementation seams, not a qualified synchronization fix. A
two-way GPU fence must gate normalization on completed producer writes and
the next producer overwrite on completed normalization, preserving each exact
allocation and its fence lifetime through resize/retirement. CPU readback,
default policy changes and GL state-custody changes are outside this checkpoint.
