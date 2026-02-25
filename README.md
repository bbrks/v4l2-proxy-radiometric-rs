# v4l2-proxy-radiometric-rs

Proxies a v4l2 camera containing radiometric (thermal) data and writes a tonemapped thermal image (with optional overlays) back to a v4l2 loopback camera output

Extracts per-pixel radiometric data from the data frame, applies color palettes and overlays, and outputs to a v4l2loopback virtual camera — usable in any app that reads from a webcam (e.g. OBS, browsers, screen recorders, Zoom...).

## Screenshots

### Before (raw input `/dev/video0`)

<img width="290" height="398" alt="Screenshot_20260225_225558" src="https://github.com/user-attachments/assets/98ad5cbb-8c7b-4c65-b4b0-ef9d5a89a284" />

### After (processed output `/dev/video2`)

`--tui` with `ffplay /dev/video2`

<img width="1386" height="544" alt="Screenshot_20260225_225504" src="https://github.com/user-attachments/assets/80ff23ca-5e71-4af5-94a5-c9e747de14d2" />

`--palette=all` preview

<img width="770" height="638" alt="Screenshot_20260225_225700" src="https://github.com/user-attachments/assets/111a9a37-6eb7-48f3-a704-f8efc87207cc" />


## Usage

- Linux only (sorry, this *heavily* relies on v4l2)
- [v4l2loopback](https://github.com/umlaeute/v4l2loopback)

Create the loopback target device:

```sh
sudo modprobe v4l2loopback devices=1 video_nr=2 card_label="Thermal Camera" exclusive_caps=1
```

Run the proxy, with the various palette or overlay settings you want to see...

```sh
v4l2-thermal-proxy-rs [OPTIONS]
```

### Options

| Flag                        | Description                                      | Default       |
|-----------------------------|--------------------------------------------------|---------------|
| `-i, --input-device <DEV>`  | Input V4L2 device                                | `/dev/video0` |
| `-o, --output-device <DEV>` | Output v4l2loopback device                       | `/dev/video2` |
| `-p, --palette <PALETTE>`   | Color palette                                    | `ironbow`     |
| `-s, --scale <N>`           | Upscale factor (default 4 gives 640x480)         | `4`           |
| `--overlay <LEVEL>`         | Overlay level (`none`, `range`, `target`, `all`) | `all`         |
| `-v, --verbose`             | Verbose output (driver details, per-frame debug) |               |
| `--tui`                     | Interactive TUI display on stderr                |               |

### Palettes

`ironbow`, `rainbow`, `grayscale`, `inverted`, `hot`, `arctic`, or `all` (which renders a 3x2 mosaic of all palettes - recommend using a higher scale to accommodate)

## How it works and what was tested with this

I tested my NOYAFA NF-583, which is a 160x120 USB-C thermal camera, designed to be used with a proprietary and frankly very sketchy looking Android App.
Ever since I wrote my [blog post about this neat camera](https://bbrks.me/noyafa-nf-583-review/) in 2022, I've wanted the ability to use this in Linux for fun and experiments.

If I plug the camera in, there's a standard 160x120 greyscale camera that is usable, but without any actual temperature information.
There's also a hidden 160x240 feed which appends a raw 16-bit radiometric data frame (an actual temperature measurement for every pixel in the image) in the bottom half of the image.
This data can be used to tonemap and provide actual per-pixel temperature overlays onto the image.

`v4l2-thermal-proxy` ignores the pre-colorized image in the top half of the frame, and instead reads the raw radiometric data, and tonemaps the temperature readings based on the color palette, with optional overlays showing min/max/center temperature points.
The result is written to a v4l2loopback device as a standard video stream which can be accessed by any software capable of using a webcam.

## Building

```sh
cargo build --release
```
