# grafting-frame

`grafting-frame` is the wgpu-free ownership boundary for native GPU frames.
It gives browser and renderer producers move-owned custody types for DX12
shared resources, Linux DMABUF images, Metal textures, IOSurfaces, and their
native synchronization resources.

GPU import policy stays in [`grafting`](https://crates.io/crates/grafting).
Most applications should use that higher-level crate. Engine and adapter crates
can use this package when a producer frame must cross a boundary without
depending on a particular wgpu release.
