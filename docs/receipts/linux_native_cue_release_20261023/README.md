# Linux native provider publication — 2026-10-23

Graft 0.6.1 was published from `db2736815e9d1893b8f9372355126056093f7155`.
The downloaded archive identifies that commit and every included Rust source
matches it. Wgpu Cue 0.1.2 receives registry Graft and adds the shared Vulkan
importer; its 12 Linux receiving tests pass, as do nine Pelt receiving tests.
The original lower provider receipt remains immutable. The standalone Iced
workspace needed both local Graft and grafting-frame version rows refreshed;
its Windows CI gate remains distinct from the executed Linux GPU qualification.
The canonical implementation plan records the final product receiving outcome.
