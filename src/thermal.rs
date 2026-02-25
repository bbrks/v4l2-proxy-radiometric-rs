// Extract temperatures from the bottom half of a YUYV frame.
//
// The bottom rows contain raw 16-bit LE radiometric data packed into
// YUYV byte positions. Formula: raw / 64.0 - 273.15 (1/64 Kelvin units).
#[cfg(test)]
fn extract_temperatures(frame: &[u8], width: usize, height: usize) -> Vec<f32> {
    let pixel_count = width * height;
    let bottom_start = pixel_count * 2; // byte offset past the colorized top half

    let bottom = &frame[bottom_start..];

    let mut temps = Vec::with_capacity(pixel_count);
    for i in 0..pixel_count {
        let offset = i * 2;
        if offset + 1 < bottom.len() {
            let raw = u16::from_le_bytes([bottom[offset], bottom[offset + 1]]);
            temps.push(raw as f32 / 64.0 - 273.15);
        } else {
            temps.push(0.0);
        }
    }
    temps
}

// Normalize temperatures to 0-255 indices.
//
// Returns (indices, t_min, t_max).
#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn tonemap(temps: &[f32]) -> (Vec<u8>, f32, f32) {
    let t_min = temps.iter().copied().fold(f32::INFINITY, f32::min);
    let t_max = temps.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let t_range = (t_max - t_min).max(0.01);

    let indices: Vec<u8> = temps
        .iter()
        .map(|&t| {
            let normalized = ((t - t_min) / t_range).clamp(0.0, 1.0);
            (normalized * 255.0) as u8
        })
        .collect();

    (indices, t_min, t_max)
}

// Extract temperatures into a pre-allocated slice (no allocation).
//
// `bottom_start` is the byte offset where raw radiometric data begins
// (i.e. `therm_w * therm_h * 2` for a dual-frame layout).
pub(crate) fn extract_temperatures_into(frame: &[u8], temps: &mut [f32], bottom_start: usize) {
    debug_assert!(
        bottom_start <= frame.len(),
        "bottom_start ({bottom_start}) out of bounds (frame len {})",
        frame.len()
    );
    let bottom = &frame[bottom_start..];

    for (i, temp) in temps.iter_mut().enumerate() {
        let offset = i * 2;
        *temp = if offset + 1 < bottom.len() {
            let raw = u16::from_le_bytes([bottom[offset], bottom[offset + 1]]);
            f32::from(raw) / 64.0 - 273.15
        } else {
            0.0
        };
    }
}

// Normalize temperatures into a pre-allocated slice (no allocation).
//
// Single pass for min/max, then normalize. Returns (`t_min`, `t_max`).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn tonemap_into(temps: &[f32], indices: &mut [u8]) -> (f32, f32) {
    let mut t_min = f32::INFINITY;
    let mut t_max = f32::NEG_INFINITY;
    for &t in temps {
        if t < t_min {
            t_min = t;
        }
        if t > t_max {
            t_max = t;
        }
    }
    let t_range = (t_max - t_min).max(0.01);

    for (i, &t) in temps.iter().enumerate() {
        let normalized = ((t - t_min) / t_range).clamp(0.0, 1.0);
        indices[i] = (normalized * 255.0) as u8;
    }

    (t_min, t_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: usize = 160;
    const HEIGHT: usize = 120;
    const PIXEL_COUNT: usize = WIDTH * HEIGHT;

    #[test]
    fn test_temperature_formula() {
        // 20°C = 293.15 K → raw = 293.15 * 64 = 18761.6 → 18762
        let raw: u16 = 18762;
        let temp = raw as f32 / 64.0 - 273.15;
        assert!((temp - 20.0).abs() < 0.02);
    }

    #[test]
    fn test_extract_temperatures() {
        // Build a fake 160x240 YUYV frame (76800 bytes)
        let mut frame = vec![0u8; WIDTH * HEIGHT * 2 * 2];

        // Put a known raw value at pixel (0,0) of the bottom half
        // 20°C → raw 18762 → LE bytes: 18762 % 256 = 90, 18762 / 256 = 73
        let raw: u16 = 18762;
        let bottom_start = WIDTH * HEIGHT * 2;
        frame[bottom_start] = (raw & 0xFF) as u8;
        frame[bottom_start + 1] = (raw >> 8) as u8;

        let temps = extract_temperatures(&frame, WIDTH, HEIGHT);
        assert_eq!(temps.len(), PIXEL_COUNT);
        assert!((temps[0] - 20.0).abs() < 0.02);
    }

    #[test]
    fn test_tonemap() {
        let temps = vec![0.0, 50.0, 100.0];
        let (indices, t_min, t_max) = tonemap(&temps);
        assert_eq!(t_min, 0.0);
        assert_eq!(t_max, 100.0);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 127); // 50/100 * 255 ≈ 127
        assert_eq!(indices[2], 255);
    }

    #[test]
    fn test_extract_temperatures_into() {
        let mut frame = vec![0u8; WIDTH * HEIGHT * 2 * 2];
        let raw: u16 = 18762;
        let bottom_start = WIDTH * HEIGHT * 2;
        frame[bottom_start] = (raw & 0xFF) as u8;
        frame[bottom_start + 1] = (raw >> 8) as u8;

        // Compare allocating vs in-place
        let expected = extract_temperatures(&frame, WIDTH, HEIGHT);
        let mut temps = vec![0.0f32; PIXEL_COUNT];
        extract_temperatures_into(&frame, &mut temps, bottom_start);
        assert_eq!(temps, expected);
    }

    #[test]
    fn test_tonemap_into() {
        let temps = vec![0.0, 50.0, 100.0];
        let (expected_indices, expected_min, expected_max) = tonemap(&temps);

        let mut indices = vec![0u8; temps.len()];
        let (t_min, t_max) = tonemap_into(&temps, &mut indices);
        assert_eq!(t_min, expected_min);
        assert_eq!(t_max, expected_max);
        assert_eq!(indices, expected_indices);
    }
}
