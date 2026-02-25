mod capture;
mod convert;
mod loopback;
mod overlay;
mod palette;
mod thermal;
mod tui;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use image::{Rgb, RgbImage};
use loopback::Loopback;
use overlay::OverlayLevel;
use palette::{PALETTE_NAMES, Palette};
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;

#[derive(Clone, Copy)]
struct ThermalConfig {
    therm_w: usize,
    therm_h: usize,
}

impl ThermalConfig {
    const fn pixel_count(self) -> usize {
        self.therm_w * self.therm_h
    }
    const fn expected_frame_bytes(self) -> usize {
        self.therm_w * self.therm_h * 2 * 2
    }
    const fn bottom_start(self) -> usize {
        self.therm_w * self.therm_h * 2
    }
    const fn center_index(self) -> usize {
        (self.therm_h / 2) * self.therm_w + (self.therm_w / 2)
    }
}

#[derive(Parser)]
#[command(
    name = "v4l2-thermal-proxy",
    about = "Thermal camera V4L2 loopback proxy"
)]
struct Args {
    /// Input V4L2 device
    #[arg(short = 'i', long = "input-device", default_value = "/dev/video0")]
    input_device: String,

    /// Output v4l2loopback device
    #[arg(short = 'o', long = "output-device", default_value = "/dev/video2")]
    output_device: String,

    /// Color palette
    #[arg(
        short, long, default_value = "ironbow",
        value_parser = clap::builder::PossibleValuesParser::new(PALETTE_NAMES),
    )]
    palette: String,

    /// Upscale factor (default 4 gives 640x480)
    #[arg(short, long, default_value_t = 4)]
    scale: u32,

    /// Overlay level
    #[arg(long, default_value = "all", value_enum)]
    overlay: OverlayLevel,

    /// Verbose output (driver details, per-frame debug)
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Interactive TUI display on stderr
    #[arg(long = "tui")]
    tui: bool,
}

// Run the warmup sequence: wait for the camera sensor to produce plausible
// temperature readings. The sensor returns all-zero frames initially, then
// bogus high temperatures (e.g. 238 C) before stabilizing.
fn run_warmup(stream: &mut Stream, verbose: bool, cfg: ThermalConfig) -> Result<()> {
    eprint!("Warming up camera");
    let mut warmup_temps = vec![0.0f32; cfg.pixel_count()];
    let mut dot_time = Instant::now();
    for i in 0..250 {
        let (buf, _meta) = stream.next()?;
        if buf.len() < cfg.expected_frame_bytes() {
            continue;
        }

        thermal::extract_temperatures_into(buf, &mut warmup_temps, cfg.bottom_start());
        let center = warmup_temps[cfg.center_index()];
        let plausible = center > -40.0 && center < 85.0;

        if verbose && dot_time.elapsed().as_millis() >= 500 {
            eprint!("({center:.0}C)");
            std::io::stderr().flush().ok();
            dot_time = Instant::now();
        } else if dot_time.elapsed().as_millis() >= 500 {
            eprint!(".");
            std::io::stderr().flush().ok();
            dot_time = Instant::now();
        }

        if plausible {
            if verbose {
                eprint!(" ready (frame {i}, center={center:.1}C)");
            } else {
                eprint!(" ready");
            }
            eprintln!();
            return Ok(());
        }
    }
    eprintln!(" (timed out, continuing anyway)");
    Ok(())
}

// Process one camera frame: extract temps, tonemap, render, convert, write.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::similar_names
)]
fn render_frame(
    buf: &[u8],
    temps: &mut [f32],
    indices: &mut [u8],
    display: &mut RgbImage,
    yuyv: &mut [u8],
    loopback: &mut Loopback,
    palettes: &[Palette],
    palette: &Palette,
    mosaic_mode: bool,
    scale: u32,
    out_w: u32,
    out_h: u32,
    overlay_level: OverlayLevel,
    cfg: ThermalConfig,
) -> Result<(f32, f32)> {
    thermal::extract_temperatures_into(buf, temps, cfg.bottom_start());
    let (t_min, t_max) = thermal::tonemap_into(temps, indices);

    let tw = cfg.therm_w as u32;
    let th = cfg.therm_h as u32;

    if mosaic_mode {
        let tile_w = out_w / 3;
        let tile_h = out_h / 2;
        for (pi, pal) in palettes.iter().enumerate() {
            let tile_col = (pi % 3) as u32;
            let tile_row = (pi / 3) as u32;
            let ox0 = tile_col * tile_w;
            let oy0 = tile_row * tile_h;
            for (i, &idx) in indices.iter().enumerate() {
                let sx = (i as u32) % tw;
                let sy = (i as u32) / tw;
                let [r, g, b] = pal.lut[idx as usize];
                let color = Rgb([r, g, b]);
                let px_x0 = sx * tile_w / tw;
                let px_x1 = (sx + 1) * tile_w / tw;
                let px_y0 = sy * tile_h / th;
                let px_y1 = (sy + 1) * tile_h / th;
                for py in px_y0..px_y1 {
                    for px in px_x0..px_x1 {
                        display.put_pixel(ox0 + px, oy0 + py, color);
                    }
                }
            }
        }
    } else {
        for (i, &idx) in indices.iter().enumerate() {
            let sx = (i as u32) % tw;
            let sy = (i as u32) / tw;
            let [r, g, b] = palette.lut[idx as usize];
            let color = Rgb([r, g, b]);
            let bx = sx * scale;
            let by = sy * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    display.put_pixel(bx + dx, by + dy, color);
                }
            }
        }

        overlay::draw_overlay(
            display,
            temps,
            t_min,
            t_max,
            &palette.name,
            overlay_level,
            cfg.therm_w,
            cfg.therm_h,
        );
    }

    convert::rgb_to_yuyv_into(display, yuyv);
    loopback.write_frame(yuyv)?;

    Ok((t_min, t_max))
}

#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
fn main() -> Result<()> {
    let args = Args::parse();

    let mosaic_mode = args.palette == "all";
    let palettes: Vec<Palette> = if mosaic_mode {
        Palette::all()
    } else {
        vec![Palette::by_name(&args.palette).expect("clap validated palette name")]
    };
    let palette = &palettes[0];

    // Open camera and derive thermal resolution from actual V4L2 format
    let (dev, cam_w, cam_h) = capture::open_camera(&args.input_device, args.verbose)?;
    let cfg = ThermalConfig {
        therm_w: cam_w as usize,
        therm_h: (cam_h / 2) as usize,
    };

    let scale = args.scale;
    let out_w = cfg.therm_w as u32 * scale;
    let out_h = cfg.therm_h as u32 * scale;

    // Open loopback output
    let mut loopback = Loopback::open(&args.output_device, out_w, out_h, args.verbose)?;

    let mut stream = Stream::with_buffers(&dev, Type::VideoCapture, 4)
        .context("Failed to create mmap stream")?;

    run_warmup(&mut stream, args.verbose, cfg)?;

    eprintln!();
    if mosaic_mode {
        eprintln!("Palette: all (3x2 mosaic)");
    } else {
        eprintln!("Palette: {}", palette.name);
    }
    eprintln!("Overlay: {}", args.overlay);
    eprintln!(
        "Scale:   {}x ({}x{} -> {}x{})",
        scale, cfg.therm_w, cfg.therm_h, out_w, out_h
    );
    eprintln!();
    eprintln!("Press Ctrl+C to stop.");
    eprintln!();

    // Signal handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })?;

    // Pre-allocate buffers
    let mut temps = vec![0.0f32; cfg.pixel_count()];
    let mut indices = vec![0u8; cfg.pixel_count()];
    let mut display = RgbImage::new(out_w, out_h);
    let mut yuyv = vec![0u8; (out_w * out_h * 2) as usize];

    let tui_mode = args.tui && tui::stderr_is_tty();

    let mut fps_time = Instant::now();
    let mut fps_count: u32 = 0;
    let mut fps_display: f64 = 0.0;
    let mut interval_t_min = f32::INFINITY;
    let mut interval_t_max = f32::NEG_INFINITY;
    let mut frame_num: u64 = 0;
    let mut tui_time = Instant::now();

    while running.load(Ordering::Relaxed) {
        let frame_start = Instant::now();

        let (buf, _meta) = stream.next()?;
        if buf.len() < cfg.expected_frame_bytes() {
            continue;
        }
        frame_num += 1;

        let (t_min, t_max) = render_frame(
            buf,
            &mut temps,
            &mut indices,
            &mut display,
            &mut yuyv,
            &mut loopback,
            &palettes,
            palette,
            mosaic_mode,
            scale,
            out_w,
            out_h,
            args.overlay,
            cfg,
        )?;

        // Track interval min/max
        if t_min < interval_t_min {
            interval_t_min = t_min;
        }
        if t_max > interval_t_max {
            interval_t_max = t_max;
        }

        let proc_ms = frame_start.elapsed().as_secs_f64() * 1000.0;

        // FPS tracking
        fps_count += 1;
        let elapsed = fps_time.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            fps_display = f64::from(fps_count) / elapsed;
            fps_count = 0;
            fps_time = Instant::now();

            if args.verbose && !tui_mode {
                eprintln!("  {fps_display:.1} fps | {interval_t_min:.1}~{interval_t_max:.1} C",);
            }

            interval_t_min = f32::INFINITY;
            interval_t_max = f32::NEG_INFINITY;
        }

        // TUI: interactive full-frame display, throttled to 25 fps
        if tui_mode && tui_time.elapsed().as_millis() >= 40 {
            let tui_label = if mosaic_mode { "all" } else { &palette.name };
            tui::draw_tui_frame(
                &indices,
                &palette.lut,
                &temps,
                t_min,
                t_max,
                frame_num,
                fps_display,
                proc_ms,
                tui_label,
                cfg.therm_w,
                cfg.therm_h,
            );
            tui_time = Instant::now();
        }
    }

    eprintln!("\nShutting down...");
    Ok(())
}
