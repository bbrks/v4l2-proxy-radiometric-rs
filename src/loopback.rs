use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

use anyhow::{Context, Result};

// `_IOWR('V', 5, struct v4l2_format)` — `VIDIOC_S_FMT` for `x86_64/ARM/AArch64`.
// Encoding: direction(0xC0) | size(0x0D0 = 208) | type('V' = 0x56) | nr(0x05).
const VIDIOC_S_FMT: libc::c_ulong = 0xC0D0_5605;
const V4L2_BUF_TYPE_VIDEO_OUTPUT: u32 = 2;
const V4L2_FIELD_NONE: u32 = 1;
const V4L2_PIX_FMT_YUYV: u32 = 0x5659_5559;

// Kernel struct `v4l2_format` (208 bytes on `x86_64`).
//
// Layout: 4-byte type, 4-byte padding, then the `pix_format` union member
// (48 bytes), padded to 208 bytes total.
#[repr(C)]
struct V4l2Format {
    type_: u32,
    _padding: u32,
    // struct v4l2_pix_format (48 bytes)
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    flags: u32,
    ycbcr_enc: u32,
    quantization: u32,
    xfer_func: u32,
    // remaining union padding (208 - 4 - 4 - 48 = 152 bytes)
    _reserved: [u8; 152],
}

const _: () = assert!(std::mem::size_of::<V4l2Format>() == 208);

impl V4l2Format {
    const fn new(width: u32, height: u32) -> Self {
        Self {
            type_: V4L2_BUF_TYPE_VIDEO_OUTPUT,
            _padding: 0,
            width,
            height,
            pixelformat: V4L2_PIX_FMT_YUYV,
            field: V4L2_FIELD_NONE,
            bytesperline: width * 2,
            sizeimage: width * height * 2,
            colorspace: 0,
            priv_: 0,
            flags: 0,
            ycbcr_enc: 0,
            quantization: 0,
            xfer_func: 0,
            _reserved: [0u8; 152],
        }
    }
}

pub(crate) struct Loopback {
    file: File,
}

impl Loopback {
    pub(crate) fn open(path: &str, width: u32, height: u32, verbose: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| {
                format!(
                    "Cannot open {path}\n\n\
                     Is v4l2loopback loaded? Try:\n  \
                     sudo modprobe v4l2loopback devices=1 video_nr=2 \
                     card_label=\"Thermal Camera\" exclusive_caps=1"
                )
            })?;

        let mut fmt = V4l2Format::new(width, height);
        let ret = unsafe { libc::ioctl(file.as_raw_fd(), VIDIOC_S_FMT, &raw mut fmt) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error())
                .context("VIDIOC_S_FMT ioctl failed on loopback device");
        }

        // Read device name from sysfs
        let dev_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|name| fs::read_to_string(format!("/sys/class/video4linux/{name}/name")).ok())
            .map_or_else(|| "Unknown".into(), |s| s.trim().to_string());

        eprintln!("Output: {dev_name} ({path}) {width}x{height} YUYV");

        if verbose {
            eprintln!("        bytes/line: {}", width * 2);
            eprintln!("        frame:     {} B", width * height * 2);
        }

        let mut lb = Self { file };

        // Write one blank frame so v4l2loopback sets ready_for_capture,
        // allowing consumers to open the device.
        let blank = vec![0u8; (width * height * 2) as usize];
        lb.write_frame(&blank)?;

        Ok(lb)
    }

    pub(crate) fn write_frame(&mut self, yuyv: &[u8]) -> Result<()> {
        self.file.write_all(yuyv)?;
        Ok(())
    }
}
