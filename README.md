# minimal-image-viewer

A minimal Linux image viewer written in Rust.

## Features

- Opens `png`, `jpg`, `jpeg`, and `jxl` files.
- When one image is opened, it lists other supported images in the same folder.
- Sidebar for file selection.
- `Prev` / `Next` buttons and left/right arrow key navigation.
- Scales image to fit the window.
- Zoom with mouse wheel and pan by dragging.
- `Browse Folder` window with icon mode and list mode, both showing thumbnails.

## JXL support

JXL decoding is done through `djxl` (from libjxl tools) and then loaded as PNG.

Install on Debian/Ubuntu:

```bash
sudo apt install libjxl-tools
```

Install on Arch:

```bash
sudo pacman -S libjxl
```

## Build and run

Prerequisites:

- Rust toolchain with `cargo`
- `libjxl-tools` for JXL support
- GTK and native GUI dependencies required by `eframe`/`egui` on your distro

Build:

```bash
cargo build --release
```

Run:

```bash
cargo run --release -- /path/to/image.png
```

Or run without argument and click **Open**.
