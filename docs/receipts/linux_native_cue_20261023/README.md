# Linux native Cue provider — 2026-10-23

The lower Graft producer/import path passes on the ThinkPad's hardware
radeonsi/RADV Renoir stack. Final Linux validation with Khronos validation
requested passes 10 library tests and three explicit DMA-BUF GPU tests. The new
GL fixture overwrites the source after snapshot, destroys the producer, then
checks consumer pixel orientation and half alpha on an explicit linear AB24
allocation. Existing RGBA/BGRA round trips pass too. Wgpu 29/28 source checks
pass; macOS wgpu-30 library tests pass 4/0. The concrete Servo DMA-BUF adapter
compiles against pinned upstream Servo on Linux.

The current [implementation record](../../../design_docs/2026-10-23_linux_native_cue_plan.md)
separates those gates from shared importer and full Turnstone product receiving.
Exact commands and their outcomes accompany the final Linux gates. The
macOS command is `cargo test -p grafting --no-default-features --features
wgpu-30 --lib --offline -j4`. The Servo command is `cargo check -p
servo-wgpu-interop-adapter --no-default-features --features wgpu-30,servo
--locked --offline -j2`, with the recorded shared build directory and development
debug information disabled. Warnings and Cargo's intentionally invalid Servo
tidy-fixture discovery diagnostic are retained; command outcomes remain separate.

The shared consumer receipt in Turnstone will bind the published provider cohort
to its executable, native/retained directions and real product captures. Physical
IME, human AT, physical reset and OS compositor correlation remain separate.
