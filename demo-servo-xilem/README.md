# demo-servo-xilem

Servo embedded in a [Xilem](https://github.com/linebender/xilem) reactive UI (git main, rev `dc496876`) with a URL bar and full input forwarding.

## What it demonstrates

- Zero-copy GPU import. The viewport is a small `external_surface` widget that Masonry paints as an *external layer*: it records only the widget's bounds. grafting imports each Servo frame onto Masonry's own wgpu device, and the demo's vendored `masonry_winit` copies that texture into the layer's rectangle before presenting.
- CPU readback as the fallback: if the import can't be set up, frames are read back and uploaded into the same texture slot, so the rest of the demo is unchanged.
- The status line delivered through a `tokio::sync::watch` channel and Xilem `task_raw`
- URL bar with navigation on Enter
- Mouse, scroll, and keyboard events in the viewport forwarded to Servo, mapped through the external layer's rectangle

## Usage

```bash
cargo run -p demo-servo-xilem                         # built-in animated fixture
cargo run -p demo-servo-xilem -- https://example.com  # load a URL
```

## Why Xilem main and a vendored masonry_winit

- The Xilem 0.4.0 release (2025-10) renders with vello 0.6 on wgpu 26, below grafting's wgpu 28–30. Main is on wgpu 28.
- Masonry main already models external content (`PaintLayerMode::External`, `VisualLayerKind::External`), documented as existing "before host integration lands". Its window host, `masonry_winit`, still drops those layers, and it renders and presents privately. Xilem's driver only runs inside `masonry_winit`'s `MasonryState`, so a host of our own cannot drive Xilem.
- So `masonry_winit/` here is upstream's crate at the same rev (Apache-2.0), substituted through the root workspace's `[patch]`. The changes are marked `wgpu-graft:` in `src/event_loop_runner.rs`:
  - collect the first external layer's rectangle instead of dropping it;
  - copy a registered texture into it before presenting;
  - add accessors for the device, redraws and scale factor.
- Upstream records an external layer's bounds in the widget's own coordinates, so the host takes the window position from the widget's window transform, looked up by id.
- Masonry resets a widget's layer mode every paint pass and drops an unrepainted widget from the next plan, so the surface widget repaints every frame. The page is live content, so that costs nothing extra.
- To move to a newer Xilem: bump the rev, copy `masonry_winit` from it, and reapply the marked hunks.

## Platform notes

- GPU import on macOS (Metal), Linux (Vulkan) and Windows (DX12 through ANGLE's shared texture), on Masonry's own device.
- **Windows**: requires ANGLE DLLs next to the executable (see [workspace README](../README.md#prerequisites)).

## License

[MPL-2.0](../LICENSE); `masonry_winit/` is Apache-2.0 (see its `LICENSE`).
