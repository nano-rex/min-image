# minimal-image-viewer

A minimal Linux image viewer written in Rust.

## Features

- Opens `png`, `jpg`, `jpeg`, and `jxl` files.
- When one image is opened, it lists other supported images in the same folder.
- Sidebar for file selection.
- `Prev` / `Next` buttons and left/right arrow key navigation.
- Scales image to fit the window.

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

```bash
cargo run --release -- /path/to/image.png
```

Or run without argument and click **Open**.
