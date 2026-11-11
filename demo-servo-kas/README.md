# demo-servo-kas

Servo embedded in a [Kas](https://github.com/kas-gui/kas) 0.17 application, zero-copy, through Kas's public custom-pipe API.

## What it demonstrates

- Kas's wgpu front end takes a *custom pipe* (`kas_wgpu::draw::CustomPipeBuilder` / `CustomPipe`). Application code gets Kas's own wgpu device when the pipe is built, and the queue on resize, and draws inside Kas's render pass. No Kas patch is needed.
- Servo starts on Kas's device on the first resize. Each frame the pipe imports Servo's output through grafting as a `wgpu::Texture` and draws it as a textured quad over the `ServoView` widget's rectangle.
- Kas renders into `Bgra8UnormSrgb`, so the fragment shader decodes Servo's sRGB-encoded bytes to linear before writing them out.
- `Draw::animate` keeps the view redrawing every frame, as live web content needs.
- Mouse, scroll and keyboard events on the view are forwarded to Servo.

## Usage

```bash
cargo run -p demo-servo-kas                         # built-in animated fixture
cargo run -p demo-servo-kas -- https://example.com  # load a URL
```

## Notes

- Kas 0.17 is on wgpu 28, so the adapter uses grafting's `wgpu-28` row.
- Kas 0.17 builds on winit `0.31.0-beta.2`. `keyutils.rs` is the shared key mapping adjusted for it: Super keys are Meta there, and space arrives as a character.
- No URL bar yet; pass a URL on the command line.
- No CPU fallback: if the import can't be set up, the view stays empty and the reason is logged.
- **Windows**: forces DX12 and the high-performance GPU through Kas's `KAS_BACKENDS` and `KAS_POWER_PREFERENCE` variables (Kas ignores `WGPU_BACKEND` and defaults to low power; ANGLE could not be matched to the integrated GPU on a hybrid laptop), and requires ANGLE DLLs next to the executable (see [workspace README](../README.md#prerequisites)).

## License

[MPL-2.0](../LICENSE)
