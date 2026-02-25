use image::RgbImage;

// Convert an RGB image to packed YUYV bytes for V4L2 output.
//
// For each pixel pair (p0, p1):
//   Y0 = 0.299*R0 + 0.587*G0 + 0.114*B0
//   Y1 = 0.299*R1 + 0.587*G1 + 0.114*B1
//   U  = -0.169*R0 - 0.331*G0 + 0.500*B0 + 128
//   V  =  0.500*R0 - 0.419*G0 - 0.081*B0 + 128
// Output: [Y0, U, Y1, V] per pair
#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rgb_to_yuyv(img: &RgbImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut yuyv = Vec::with_capacity((w * h * 2) as usize);

    for y in 0..h {
        for x in (0..w).step_by(2) {
            let p0 = img.get_pixel(x, y).0;
            let p1 = if x + 1 < w {
                img.get_pixel(x + 1, y).0
            } else {
                p0
            };

            let (r0, g0, b0) = (p0[0] as f32, p0[1] as f32, p0[2] as f32);
            let (r1, g1, b1) = (p1[0] as f32, p1[1] as f32, p1[2] as f32);

            let y0 = (0.299 * r0 + 0.587 * g0 + 0.114 * b0).clamp(0.0, 255.0) as u8;
            let y1 = (0.299 * r1 + 0.587 * g1 + 0.114 * b1).clamp(0.0, 255.0) as u8;
            let u = (-0.169 * r0 - 0.331 * g0 + 0.500 * b0 + 128.0).clamp(0.0, 255.0) as u8;
            let v = (0.500 * r0 - 0.419 * g0 - 0.081 * b0 + 128.0).clamp(0.0, 255.0) as u8;

            yuyv.push(y0);
            yuyv.push(u);
            yuyv.push(y1);
            yuyv.push(v);
        }
    }

    yuyv
}

// Convert an RGB image to packed YUYV bytes, writing into a pre-allocated buffer.
//
// Uses fixed-point integer arithmetic (BT.601 coefficients × 256) and raw
// pixel buffer access to avoid per-pixel bounds checks and float operations.
#[allow(clippy::many_single_char_names, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn rgb_to_yuyv_into(img: &RgbImage, out: &mut [u8]) {
    let (w, h) = img.dimensions();
    debug_assert_eq!(out.len(), (w * h * 2) as usize, "output buffer size mismatch");
    let raw = img.as_raw();
    let stride = (w * 3) as usize;
    let mut oi = 0;

    for y in 0..h as usize {
        let row = y * stride;
        let mut x = 0usize;
        while x < w as usize {
            let p0 = row + x * 3;
            let r0 = i32::from(raw[p0]);
            let g0 = i32::from(raw[p0 + 1]);
            let b0 = i32::from(raw[p0 + 2]);

            let (r1, g1, b1) = if x + 1 < w as usize {
                let p1 = p0 + 3;
                (i32::from(raw[p1]), i32::from(raw[p1 + 1]), i32::from(raw[p1 + 2]))
            } else {
                (r0, g0, b0)
            };

            // Full-range BT.601 coefficients × 256
            let y0 = ((77 * r0 + 150 * g0 + 29 * b0 + 128) >> 8).clamp(0, 255) as u8;
            let y1 = ((77 * r1 + 150 * g1 + 29 * b1 + 128) >> 8).clamp(0, 255) as u8;
            let u = (((-43 * r0 - 85 * g0 + 128 * b0 + 128) >> 8) + 128).clamp(0, 255) as u8;
            let v = (((128 * r0 - 107 * g0 - 21 * b0 + 128) >> 8) + 128).clamp(0, 255) as u8;

            out[oi] = y0;
            out[oi + 1] = u;
            out[oi + 2] = y1;
            out[oi + 3] = v;
            oi += 4;

            x += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn test_rgb_to_yuyv_size() {
        let img = RgbImage::new(4, 2);
        let yuyv = rgb_to_yuyv(&img);
        // 4 * 2 * 2 = 16 bytes
        assert_eq!(yuyv.len(), 16);
    }

    #[test]
    fn test_white_pixel() {
        // White RGB(255,255,255) → Y≈235, U≈128, V≈128 (BT.601)
        let mut img = RgbImage::new(2, 1);
        img.put_pixel(0, 0, Rgb([255, 255, 255]));
        img.put_pixel(1, 0, Rgb([255, 255, 255]));
        let yuyv = rgb_to_yuyv(&img);
        // Y should be close to 255 (full-range), U and V close to 128
        assert!(yuyv[0] > 240); // Y0
        assert!((yuyv[1] as i16 - 128).unsigned_abs() < 5); // U
        assert!(yuyv[2] > 240); // Y1
        assert!((yuyv[3] as i16 - 128).unsigned_abs() < 5); // V
    }

    #[test]
    fn test_black_pixel() {
        // Black RGB(0,0,0) → Y≈0, U≈128, V≈128
        let mut img = RgbImage::new(2, 1);
        img.put_pixel(0, 0, Rgb([0, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 0, 0]));
        let yuyv = rgb_to_yuyv(&img);
        assert!(yuyv[0] < 5); // Y0
        assert_eq!(yuyv[1], 128); // U
        assert!(yuyv[2] < 5); // Y1
        assert_eq!(yuyv[3], 128); // V
    }

    #[test]
    fn test_rgb_to_yuyv_into_matches_original() {
        // Build an image with varied colors
        let mut img = RgbImage::new(8, 4);
        let colors = [
            Rgb([255, 0, 0]),
            Rgb([0, 255, 0]),
            Rgb([0, 0, 255]),
            Rgb([255, 255, 0]),
            Rgb([128, 64, 32]),
            Rgb([0, 0, 0]),
            Rgb([255, 255, 255]),
            Rgb([100, 200, 50]),
        ];
        for y in 0..4u32 {
            for x in 0..8u32 {
                img.put_pixel(x, y, colors[((x + y * 8) % 8) as usize]);
            }
        }

        let expected = rgb_to_yuyv(&img);
        let mut out = vec![0u8; expected.len()];
        rgb_to_yuyv_into(&img, &mut out);

        // Integer math may differ by ±1 from float math
        for (i, (&e, &a)) in expected.iter().zip(out.iter()).enumerate() {
            assert!(
                (e as i16 - a as i16).unsigned_abs() <= 1,
                "byte {i}: expected {e}, got {a}",
            );
        }
    }
}
