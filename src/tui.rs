use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};

// Check if stderr is an interactive terminal.
pub(crate) fn stderr_is_tty() -> bool {
    io::stderr().is_terminal()
}

// Find (`min_index`, `max_index`) in temperatures.
fn find_min_max(temps: &[f32]) -> (usize, usize) {
    let mut min_i = 0;
    let mut max_i = 0;
    for (i, &t) in temps.iter().enumerate().skip(1) {
        if t < temps[min_i] {
            min_i = i;
        }
        if t > temps[max_i] {
            max_i = i;
        }
    }
    (min_i, max_i)
}

struct Marker {
    ac: usize,
    ar: usize,
    symbol: &'static str,
    label: String,
    r: u8,
    g: u8,
    b: u8,
}

// Render a full TUI frame to stderr.
//
// Uses half-block characters (▀) with 24-bit truecolor to render the thermal
// image at 80×60 pixel resolution (80 cols × 30 char rows), colored to match
// the active tonemapping palette. Overlays hi/lo/center markers on the image.
#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
pub(crate) fn draw_tui_frame(
    indices: &[u8],
    lut: &[[u8; 3]; 256],
    temps: &[f32],
    t_min: f32,
    t_max: f32,
    frame_num: u64,
    fps: f64,
    proc_ms: f64,
    palette_name: &str,
    therm_w: usize,
    therm_h: usize,
) {
    let mut out = String::with_capacity(16384);

    // Move cursor home and clear screen (overwrite previous frame in-place)
    out.push_str("\x1b[H\x1b[2J");

    let center_temp = temps[(therm_h / 2) * therm_w + (therm_w / 2)];

    // ── Header ──────────────────────────────────────────────────────────

    let _ = writeln!(
        out,
        "\x1b[1m\u{2501}\u{2501}\u{2501} Frame #{frame_num:<6} \u{2501}\u{2501}\u{2501} {fps:.1} fps \u{2501}\u{2501}\u{2501} proc {proc_ms:.2}ms \u{2501}\u{2501}\u{2501} {palette_name} \u{2501}\u{2501}\u{2501}\x1b[0m",
    );

    // ── Palette-colored thermal image using half-block characters ────────
    //
    // Each character cell renders two vertical pixels using ▀ (upper half block)
    // with fg = top pixel color and bg = bottom pixel color.
    // 80 cols × 30 rows = 80×60 pixel resolution (half of 160×120).

    let art_cols: usize = (therm_w / 2).max(1);
    let art_rows: usize = (therm_h / 4).max(1); // character rows; each renders 2 pixel rows
    let col_step = therm_w / art_cols;
    let row_step = therm_h / (art_rows * 2);

    for cr in 0..art_rows {
        out.push_str("  ");
        for cc in 0..art_cols {
            let tx = cc * col_step;
            let ty_top = (cr * 2) * row_step;
            let ty_bot = (cr * 2 + 1) * row_step;

            let [rt, gt, bt] = lut[indices[ty_top * therm_w + tx] as usize];
            let [rb, gb, bb] = lut[indices[ty_bot * therm_w + tx] as usize];

            // fg = top pixel, bg = bottom pixel, char = ▀
            let _ = write!(out, "\x1b[38;2;{rt};{gt};{bt};48;2;{rb};{gb};{bb}m\u{2580}",);
        }
        out.push_str("\x1b[0m\n");
    }

    // ── Overlay: hi/lo/center markers on the image ──────────────────────
    //
    // Use ANSI cursor positioning to overwrite cells at marker locations.
    // Image starts at terminal row 2, column 3 (1-indexed).

    let (min_i, max_i) = find_min_max(temps);

    let markers = [
        Marker {
            ac: (min_i % therm_w) / col_step,
            ar: (min_i / therm_w) / (row_step * 2),
            symbol: "\u{25be}", // ▾
            label: format!("{t_min:.1}\u{00b0}"),
            r: 0,
            g: 128,
            b: 255,
        },
        Marker {
            ac: (max_i % therm_w) / col_step,
            ar: (max_i / therm_w) / (row_step * 2),
            symbol: "\u{25b4}", // ▴
            label: format!("{t_max:.1}\u{00b0}"),
            r: 255,
            g: 0,
            b: 0,
        },
        Marker {
            ac: (therm_w / 2) / col_step,
            ar: (therm_h / 2) / (row_step * 2),
            symbol: "+",
            label: format!("{center_temp:.1}\u{00b0}"),
            r: 255,
            g: 255,
            b: 255,
        },
    ];

    for m in &markers {
        let term_row = m.ar + 2; // image starts at terminal row 2
        let label_text = format!("{}{}", m.symbol, m.label);
        let label_len = label_text.chars().count();

        // Place label to the right if it fits, otherwise to the left
        let (term_col, text) = if m.ac + label_len < art_cols {
            (m.ac + 3, label_text) // +3 for "  " indent (1-indexed)
        } else {
            let start = m.ac.saturating_sub(label_len) + 3;
            (start, label_text)
        };

        let _ = write!(
            out,
            "\x1b[{};{}H\x1b[1;38;2;{};{};{};48;2;0;0;0m{}\x1b[0m",
            term_row, term_col, m.r, m.g, m.b, text,
        );
    }

    // Move cursor below the image for the footer
    let footer_row = art_rows + 2;
    let _ = write!(out, "\x1b[{footer_row};1H");

    // ── Footer: summary stats ───────────────────────────────────────────

    let _ = write!(
        out,
        "\n  Range: \x1b[34m{:.1}\x1b[0m\u{00b0}C ~ \x1b[31m{:.1}\x1b[0m\u{00b0}C  |  \u{0394}: {:.1}\u{00b0}C  |  Center: {:.1}\u{00b0}C\n",
        t_min,
        t_max,
        t_max - t_min,
        center_temp,
    );
    out.push_str(
        "\x1b[1m\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\x1b[0m\n",
    );

    eprint!("{out}");
    io::stderr().flush().ok();
}
