pub(crate) const PALETTE_NAMES: &[&str] = &[
    "ironbow", "rainbow", "grayscale", "inverted", "hot", "arctic", "all",
];

pub(crate) struct Palette {
    pub(crate) name: String,
    pub(crate) lut: [[u8; 3]; 256],
}

impl Palette {
    pub(crate) fn by_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        let lut = match lower.as_str() {
            "ironbow" => ironbow(),
            "rainbow" => rainbow(),
            "grayscale" => grayscale(),
            "inverted" => inverted(),
            "hot" => hot(),
            "arctic" => arctic(),
            _ => return None,
        };
        Some(Self { name: lower, lut })
    }

    // Return all 6 palettes in display order.
    pub(crate) fn all() -> Vec<Self> {
        ["ironbow", "rainbow", "grayscale", "inverted", "hot", "arctic"]
            .iter()
            .map(|name| Self::by_name(name).unwrap())
            .collect()
    }
}

// Linearly interpolate between RGB control points to build a 256-entry LUT.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn interpolate(control_points: &[(u8, [u8; 3])]) -> [[u8; 3]; 256] {
    let mut lut = [[0u8; 3]; 256];
    for pair in control_points.windows(2) {
        let (pos0, rgb0) = pair[0];
        let (pos1, rgb1) = pair[1];
        let n = (pos1 - pos0) as usize;
        if n == 0 {
            continue;
        }
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let idx = pos0 as usize + i;
            for ch in 0..3 {
                lut[idx][ch] =
                    (f32::from(rgb1[ch]) - f32::from(rgb0[ch])).mul_add(t, f32::from(rgb0[ch])).clamp(0.0, 255.0) as u8;
            }
        }
    }
    lut
}

// All control points are RGB (converted from Python's BGR originals).

fn ironbow() -> [[u8; 3]; 256] {
    interpolate(&[
        (0, [0, 0, 0]),         // black
        (32, [0, 0, 128]),      // dark blue
        (64, [64, 0, 196]),     // blue-magenta
        (96, [128, 0, 128]),    // magenta
        (128, [196, 0, 0]),     // red
        (160, [255, 64, 0]),    // orange
        (192, [255, 128, 0]),   // orange-yellow
        (224, [255, 220, 0]),   // yellow
        (255, [255, 255, 255]), // white
    ])
}

fn rainbow() -> [[u8; 3]; 256] {
    interpolate(&[
        (0, [0, 0, 255]),       // blue
        (64, [0, 255, 255]),    // cyan
        (128, [0, 255, 0]),     // green
        (192, [255, 255, 0]),   // yellow
        (255, [255, 0, 0]),     // red
    ])
}

#[allow(clippy::cast_possible_truncation)]
fn grayscale() -> [[u8; 3]; 256] {
    let mut lut = [[0u8; 3]; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let v = i as u8;
        *entry = [v, v, v];
    }
    lut
}

#[allow(clippy::cast_possible_truncation)]
fn inverted() -> [[u8; 3]; 256] {
    let mut lut = [[0u8; 3]; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let v = (255 - i) as u8;
        *entry = [v, v, v];
    }
    lut
}

fn hot() -> [[u8; 3]; 256] {
    interpolate(&[
        (0, [0, 0, 0]),         // black
        (85, [255, 0, 0]),      // red
        (170, [255, 255, 0]),   // yellow
        (255, [255, 255, 255]), // white
    ])
}

fn arctic() -> [[u8; 3]; 256] {
    interpolate(&[
        (0, [0, 32, 128]),      // dark blue
        (64, [0, 128, 255]),    // medium blue
        (128, [200, 255, 255]), // light cyan/white
        (192, [255, 255, 0]),   // yellow
        (255, [255, 0, 0]),     // red
    ])
}
