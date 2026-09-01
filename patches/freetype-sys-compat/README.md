# freetype-sys compatibility package

`zed-font-kit` requests `freetype-sys` 0.20 directly on Unix. Servo 0.5 uses
0.23, and Cargo rejects two packages that both declare `links = "freetype"`.

This package satisfies the 0.20 version edge without declaring a native link,
then re-exports 0.23. The upstream 0.23 package remains the sole owner of the
native FreeType build and link metadata.
