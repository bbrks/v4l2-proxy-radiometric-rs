use anyhow::{Context, Result};
use v4l::video::Capture;
use v4l::{Device, FourCC};

// Open the thermal camera and query its YUYV format.
//
// Returns `(Device, width, height)` where width/height are the camera's
// native frame dimensions. No resolution is hardcoded — the caller derives
// thermal dimensions from `height / 2`.
pub(crate) fn open_camera(path: &str, verbose: bool) -> Result<(Device, u32, u32)> {
    let dev = Device::with_path(path).with_context(|| format!("Cannot open {path}"))?;

    // Ensure YUYV pixel format but don't force a specific resolution.
    let mut fmt = dev.format()?;
    fmt.fourcc = FourCC::new(b"YUYV");
    dev.set_format(&fmt)?;

    let actual = dev.format()?;

    // Query device capabilities for the card name
    let caps = dev.query_caps().ok();
    let card = caps
        .as_ref()
        .map_or_else(|| "Unknown".into(), |c| c.card.clone());

    eprintln!(
        "Input:  {} ({}) {}x{} YUYV",
        card.trim(),
        path,
        actual.width,
        actual.height,
    );

    if verbose {
        if let Some(caps) = &caps {
            eprintln!("        driver:  {}", caps.driver);
            eprintln!("        bus:     {}", caps.bus);
            eprintln!(
                "        version: {}.{}.{}",
                caps.version.0, caps.version.1, caps.version.2,
            );
        }
        eprintln!("        stride:  {} B/line", actual.stride);
        eprintln!("        frame:   {} B", actual.size);
    }

    if actual.height % 2 != 0 {
        eprintln!(
            "WARNING: Frame height {} is odd — dual-frame layout requires even height",
            actual.height,
        );
    }

    Ok((dev, actual.width, actual.height))
}
