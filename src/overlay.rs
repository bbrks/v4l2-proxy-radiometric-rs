use clap::ValueEnum;
use font8x8::UnicodeFonts;
use image::{Rgb, RgbImage};

const WHITE: [u8; 3] = [255, 255, 255];
const MIN_COLOR: [u8; 3] = [0, 128, 255]; // blue (was BGR 255,128,0 in Python)
const MAX_COLOR: [u8; 3] = [255, 0, 0]; // red (was BGR 0,0,255 in Python)

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OverlayLevel {
    // No overlay
    None,
    // Min/max markers with temperatures
    Range,
    // Min/max markers + center crosshair with temperature
    Target,
    // Full overlay: markers, crosshair, range, palette name
    All,
}

impl std::fmt::Display for OverlayLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Range => write!(f, "range"),
            Self::Target => write!(f, "target"),
            Self::All => write!(f, "all"),
        }
    }
}

#[allow(clippy::cast_sign_loss)]
fn put_pixel_safe(img: &mut RgbImage, x: i32, y: i32, color: [u8; 3]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, Rgb(color));
    }
}

fn draw_hline(img: &mut RgbImage, x0: i32, x1: i32, y: i32, color: [u8; 3]) {
    for x in x0..=x1 {
        put_pixel_safe(img, x, y, color);
    }
}

fn draw_vline(img: &mut RgbImage, x: i32, y0: i32, y1: i32, color: [u8; 3]) {
    for y in y0..=y1 {
        put_pixel_safe(img, x, y, color);
    }
}

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn draw_char(img: &mut RgbImage, x: i32, y: i32, ch: char, color: [u8; 3]) {
    if let Some(glyph) = font8x8::BASIC_FONTS.get(ch) {
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8i32 {
                if bits & (1 << col) != 0 {
                    put_pixel_safe(img, x + col, y + row as i32, color);
                }
            }
        }
    }
}

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn draw_text(img: &mut RgbImage, x: i32, y: i32, text: &str, color: [u8; 3]) {
    for (i, ch) in text.chars().enumerate() {
        draw_char(img, x + (i as i32) * 8, y, ch, color);
    }
}

// Draw an upward-pointing triangle marker (for max temperature).
fn draw_marker_up(img: &mut RgbImage, cx: i32, cy: i32, size: i32, color: [u8; 3]) {
    for dy in 0..size {
        let half_w = dy;
        draw_hline(img, cx - half_w, cx + half_w, cy - size + 1 + dy, color);
    }
}

// Draw a downward-pointing triangle marker (for min temperature).
fn draw_marker_down(img: &mut RgbImage, cx: i32, cy: i32, size: i32, color: [u8; 3]) {
    for dy in 0..size {
        let half_w = dy;
        draw_hline(img, cx - half_w, cx + half_w, cy + size - 1 - dy, color);
    }
}

// Draw temperature overlay at the given level.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names
)]
pub(crate) fn draw_overlay(
    img: &mut RgbImage,
    temps: &[f32],
    t_min: f32,
    t_max: f32,
    palette_name: &str,
    level: OverlayLevel,
    therm_w: usize,
    therm_h: usize,
) {
    if level == OverlayLevel::None {
        return;
    }

    let (dw, dh) = (img.width() as f32, img.height() as f32);
    let scale_x = dw / therm_w as f32;
    let scale_y = dh / therm_h as f32;

    // --- hi/lo: min/max markers and temperature labels ---

    let mut min_idx = 0usize;
    let mut max_idx = 0usize;
    for i in 1..temps.len() {
        if temps[i] < temps[min_idx] {
            min_idx = i;
        }
        if temps[i] > temps[max_idx] {
            max_idx = i;
        }
    }

    // Min marker (blue triangle pointing down)
    let min_dx = ((min_idx % therm_w) as f32 * scale_x) as i32;
    let min_dy = ((min_idx / therm_w) as f32 * scale_y) as i32;
    draw_marker_down(img, min_dx, min_dy, 6, MIN_COLOR);
    draw_text(
        img,
        min_dx + 10,
        min_dy,
        &format!("{t_min:.1}C"),
        MIN_COLOR,
    );

    // Max marker (red triangle pointing up)
    let max_dx = ((max_idx % therm_w) as f32 * scale_x) as i32;
    let max_dy = ((max_idx / therm_w) as f32 * scale_y) as i32;
    draw_marker_up(img, max_dx, max_dy, 6, MAX_COLOR);
    draw_text(
        img,
        max_dx + 10,
        max_dy,
        &format!("{t_max:.1}C"),
        MAX_COLOR,
    );

    // --- hi/lo/target: add center crosshair ---

    if matches!(level, OverlayLevel::Target | OverlayLevel::All) {
        let cx = therm_w / 2;
        let cy = therm_h / 2;
        let center_temp = temps[cy * therm_w + cx];
        let dcx = (cx as f32 * scale_x) as i32;
        let dcy = (cy as f32 * scale_y) as i32;

        draw_hline(img, dcx - 15, dcx - 5, dcy, WHITE);
        draw_hline(img, dcx + 5, dcx + 15, dcy, WHITE);
        draw_vline(img, dcx, dcy - 15, dcy - 5, WHITE);
        draw_vline(img, dcx, dcy + 5, dcy + 15, WHITE);

        let text = format!("{center_temp:.1}C");
        draw_text(img, dcx + 12, dcy - 12, &text, WHITE);
    }

    // --- all: add status bar with range and palette name ---

    if level == OverlayLevel::All {
        let status = format!("{palette_name}  Range: {t_min:.1}-{t_max:.1}C");
        draw_text(img, 8, dh as i32 - 14, &status, WHITE);
    }
}
