# demo-servo-gpui

Servo embedded in a [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (`gpui-pre` 0.3.10) application (Zed's UI framework) with a URL bar and full input forwarding.

## What it demonstrates

- Servo rendering offscreen via CPU readback (`read_full_frame()`)
- RGBA→BGRA pixel conversion for GPUI's `RenderImage` format
- Continuous rendering via `request_animation_frame()` in the `Render` implementation
- URL bar with focus management via `FocusHandle`
- Mouse, scroll, and keyboard events forwarded to Servo via GPUI's element event handlers
- Custom key mapping (GPUI uses its own key types, not winit)

## Usage

```bash
cargo run -p demo-servo-gpui                         # built-in animated fixture
cargo run -p demo-servo-gpui -- https://example.com  # load a URL
cargo run -p demo-servo-gpui -- servo.org            # auto-prefixes https://
```

## GPUI-specific notes

- GPUI is pre-1.0 and does not use winit, so this demo has its own key mapping in `keyutils.rs` rather than sharing the winit-based one used by the other demos.
- GPUI's `Pixels` type has a crate-private field, so all pixel math uses `f32::from(px)` rather than direct field access.
- GPUI comes from crates.io as Zed's weekly snapshot, `gpui-pre` (pinned
  `=0.3.10`, with `gpui-pre-platform`), aliased in `Cargo.toml` so the source
  still imports `gpui::...`.
- The workspace still carries a freetype-sys version-edge shim under
  `patches/` that aligns zed-font-kit with Servo's single `freetype-sys` 0.23
  native link.

## Platform notes

- Uses CPU readback on all platforms.
- **Windows**: requires ANGLE DLLs next to the executable (see [workspace README](../README.md#prerequisites)).

## License

[MPL-2.0](../LICENSE)
