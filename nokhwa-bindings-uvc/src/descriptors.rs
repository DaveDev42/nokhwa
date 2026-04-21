//! Parser for UVC class-specific `VideoStreaming` interface descriptors
//! (§3.9 of the USB Video Class 1.5 spec). Given the raw `extra` bytes
//! of a `VideoStreaming` interface alt setting, walks the chain of
//! `VS_FORMAT_*` / `VS_FRAME_*` descriptors and builds a flat list of
//! `(width, height, format, fps)` tuples that the `nokhwa` framework
//! can present through `FrameSource::compatible_formats()`.

use nokhwa_core::types::{CameraFormat, FrameFormat};

const CS_INTERFACE: u8 = 0x24;

const VS_FORMAT_UNCOMPRESSED: u8 = 0x04;
const VS_FRAME_UNCOMPRESSED: u8 = 0x05;
const VS_FORMAT_MJPEG: u8 = 0x06;
const VS_FRAME_MJPEG: u8 = 0x07;

/// A discovered format + its frame descriptors.
#[derive(Debug, Clone)]
struct FormatSpec {
    format: FrameFormat,
}

/// Decode a `VS_FORMAT_UNCOMPRESSED` descriptor's GUID (offset 5, 16
/// bytes) into a `FrameFormat`. Microsoft-style format GUIDs carry
/// the `FourCC` in the first four bytes of `Data1` (little-endian); the
/// remaining 12 bytes are the fixed Microsoft subtype template.
/// Unknown GUIDs return `None` so the caller can skip the format
/// rather than emit it with a wrong `FrameFormat`.
fn uncompressed_guid_to_format(guid: &[u8]) -> Option<FrameFormat> {
    if guid.len() < 16 {
        return None;
    }
    // Only look at the `FourCC` prefix — all UVC format GUIDs use the
    // same Microsoft template for the trailing 12 bytes.
    match &guid[..4] {
        b"YUY2" => Some(FrameFormat::YUYV),
        b"NV12" => Some(FrameFormat::NV12),
        _ => None,
    }
}

/// Interpret a `VS_FORMAT_*` descriptor body (including the `bLength`
/// / `bDescriptorType` / `bDescriptorSubType` prefix) and return the
/// `FrameFormat` it advertises, if we know how to decode it.
fn parse_format_descriptor(desc: &[u8]) -> Option<FormatSpec> {
    // Every class-specific interface descriptor is at least 3 bytes:
    // bLength, bDescriptorType, bDescriptorSubType.
    if desc.len() < 3 || desc[1] != CS_INTERFACE {
        return None;
    }
    match desc[2] {
        VS_FORMAT_MJPEG => Some(FormatSpec {
            format: FrameFormat::MJPEG,
        }),
        VS_FORMAT_UNCOMPRESSED => {
            // Layout: bLength, bDescriptorType, bDescriptorSubType,
            // bFormatIndex, bNumFrameDescriptors, guidFormat[16], ...
            if desc.len() < 5 + 16 {
                return None;
            }
            let guid = &desc[5..5 + 16];
            uncompressed_guid_to_format(guid).map(|f| FormatSpec { format: f })
        }
        _ => None,
    }
}

/// Interpret a `VS_FRAME_*` descriptor and emit the
/// `(width, height, interval_100ns[])` triple it carries.
///
/// The frame interval field is either:
/// - `bFrameIntervalType == 0` (continuous: `dwMinFrameInterval`,
///   `dwMaxFrameInterval`, `dwFrameIntervalStep`), or
/// - `bFrameIntervalType > 0` (discrete: that many `dwFrameInterval`
///   DWORDs).
///
/// Continuous intervals are expanded into a coarse sampling:
/// `[min, (min+max)/2, max]`. Real devices in the wild almost always
/// use the discrete form, so the continuous fallback rarely fires and
/// the sampling is good enough for surfacing a `Vec<CameraFormat>`
/// without exploding the list size.
fn parse_frame_descriptor(desc: &[u8]) -> Option<(u16, u16, Vec<u32>)> {
    if desc.len() < 3 || desc[1] != CS_INTERFACE {
        return None;
    }
    let subtype = desc[2];
    if subtype != VS_FRAME_MJPEG && subtype != VS_FRAME_UNCOMPRESSED {
        return None;
    }
    // Common prefix up to bFrameIntervalType:
    //   bLength(0) bDescriptorType(1) bDescriptorSubType(2)
    //   bFrameIndex(3) bmCapabilities(4)
    //   wWidth(5..7) wHeight(7..9)
    //   dwMinBitRate(9..13) dwMaxBitRate(13..17)
    //   dwMaxVideoFrameBufferSize(17..21)
    //   dwDefaultFrameInterval(21..25)
    //   bFrameIntervalType(25)
    //   (then either 3×DWORD continuous, or N×DWORD discrete)
    if desc.len() < 26 {
        return None;
    }
    let width = u16::from_le_bytes([desc[5], desc[6]]);
    let height = u16::from_le_bytes([desc[7], desc[8]]);
    let interval_type = desc[25];
    let tail = &desc[26..];

    let intervals: Vec<u32> = if interval_type == 0 {
        // continuous
        if tail.len() < 12 {
            return None;
        }
        let min = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
        let max = u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]);
        let _step = u32::from_le_bytes([tail[8], tail[9], tail[10], tail[11]]);
        let mid = min.saturating_add(max) / 2;
        vec![min, mid, max]
    } else {
        let expected = usize::from(interval_type) * 4;
        if tail.len() < expected {
            return None;
        }
        (0..usize::from(interval_type))
            .map(|i| {
                let off = i * 4;
                u32::from_le_bytes([tail[off], tail[off + 1], tail[off + 2], tail[off + 3]])
            })
            .collect()
    };

    Some((width, height, intervals))
}

/// Convert a UVC frame interval (in 100ns units) to an integer fps
/// value rounded to the nearest whole frame-per-second. An interval
/// of zero is treated as "unknown" and filtered out by the caller.
fn interval_to_fps(interval_100ns: u32) -> Option<u32> {
    if interval_100ns == 0 {
        return None;
    }
    let fps = (10_000_000u64 + u64::from(interval_100ns) / 2) / u64::from(interval_100ns);
    u32::try_from(fps).ok()
}

/// Walk the raw class-specific descriptor chain carried in the `extra`
/// bytes of a `VideoStreaming` interface's alt-setting descriptor and
/// return a flat `Vec<CameraFormat>` — one per
/// `(format × frame × discrete interval)` combination.
///
/// Frames whose `VS_FRAME_*` descriptor does not immediately follow a
/// known `VS_FORMAT_*` descriptor are silently skipped. Duplicate
/// `(width, height, format, fps)` tuples are removed, preserving first
/// occurrence order, so the returned list can be shown to users as-is.
#[must_use]
pub fn parse_video_streaming_formats(extra: &[u8]) -> Vec<CameraFormat> {
    let mut out: Vec<CameraFormat> = Vec::new();
    let mut current: Option<FormatSpec> = None;
    let mut i = 0usize;

    while i + 2 <= extra.len() {
        let len = extra[i] as usize;
        // A zero- or truncated-length descriptor means the chain is
        // malformed; stop here rather than spinning forever.
        if len < 3 || i + len > extra.len() {
            break;
        }
        let desc = &extra[i..i + len];
        if desc[1] == CS_INTERFACE {
            let subtype = desc[2];
            if subtype == VS_FORMAT_MJPEG || subtype == VS_FORMAT_UNCOMPRESSED {
                current = parse_format_descriptor(desc);
            } else if subtype == VS_FRAME_MJPEG || subtype == VS_FRAME_UNCOMPRESSED {
                if let (Some(spec), Some((w, h, intervals))) =
                    (current.as_ref(), parse_frame_descriptor(desc))
                {
                    for iv in intervals {
                        if let Some(fps) = interval_to_fps(iv) {
                            out.push(CameraFormat::new_from(
                                u32::from(w),
                                u32::from(h),
                                spec.format,
                                fps,
                            ));
                        }
                    }
                }
            }
        }
        i += len;
    }

    // Preserve order but drop duplicates. `CameraFormat` is not `Hash`,
    // so fall back to a linear uniqueness check — the format list is
    // small even for a rich device (≤ a few hundred entries).
    let mut seen: Vec<CameraFormat> = Vec::with_capacity(out.len());
    for f in out {
        if !seen.iter().any(|g| {
            g.resolution() == f.resolution()
                && g.format() == f.format()
                && g.frame_rate() == f.frame_rate()
        }) {
            seen.push(f);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal MJPEG format descriptor (no frames yet).
    const MJPEG_FORMAT: [u8; 11] = [
        0x0b, 0x24, 0x06, // bLength, CS_INTERFACE, VS_FORMAT_MJPEG
        0x02, // bFormatIndex
        0x01, // bNumFrameDescriptors
        0x00, // bmFlags
        0x01, // bDefaultFrameIndex
        0x00, 0x00, 0x00, 0x00, // aspect ratios + flags
    ];

    /// MJPEG 640×480 with three discrete intervals: 30fps, 15fps, 5fps.
    /// Size = 5-byte header + 2w + 2h + 3×4 bitrate/buffer + 4 default
    /// interval + 1 intervalType + 3×4 intervals = 38 bytes → bLength
    /// 0x26.
    const MJPEG_FRAME_640_480: [u8; 38] = [
        0x26, 0x24, 0x07, 0x01, 0x00, // header + bFrameIndex + bmCapabilities
        0x80, 0x02, // wWidth  = 0x0280 = 640
        0xe0, 0x01, // wHeight = 0x01e0 = 480
        0x00, 0x00, 0x00, 0x00, // dwMinBitRate
        0x00, 0x00, 0x00, 0x00, // dwMaxBitRate
        0x00, 0x00, 0x10, 0x00, // dwMaxVideoFrameBufferSize
        0x15, 0x16, 0x05, 0x00, // dwDefaultFrameInterval (30fps)
        0x03, // bFrameIntervalType (discrete, 3 entries)
        0x15, 0x16, 0x05, 0x00, // 30 fps   (333_333 × 100ns)
        0x2a, 0x2c, 0x0a, 0x00, // 15 fps   (666_666 × 100ns)
        0x80, 0x84, 0x1e, 0x00, // 5  fps (2_000_000 × 100ns)
    ];

    /// Uncompressed NV12 format descriptor — the GUID is "NV12" +
    /// the Microsoft subtype template.
    const NV12_FORMAT: [u8; 27] = [
        0x1b, 0x24, 0x04, // bLength, CS_INTERFACE, VS_FORMAT_UNCOMPRESSED
        0x03, // bFormatIndex
        0x01, // bNumFrameDescriptors
        b'N', b'V', b'1', b'2', // guidFormat[0..4]
        0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71, // remainder
        0x0c, // bBitsPerPixel
        0x01, 0x00, 0x00, 0x00, 0x00, // bDefaultFrameIndex, aspect, flags, copy
    ];

    /// Uncompressed NV12 320×240, one discrete interval (30fps).
    const NV12_FRAME_320_240: [u8; 30] = [
        0x1e, 0x24, 0x05, 0x01, 0x00, //
        0x40, 0x01, // 320
        0xf0, 0x00, // 240
        0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x10, 0x00, //
        0x15, 0x16, 0x05, 0x00, //
        0x01, // discrete, 1 entry
        0x15, 0x16, 0x05, 0x00, // 30 fps
    ];

    fn concat(slices: &[&[u8]]) -> Vec<u8> {
        slices.iter().flat_map(|s| s.iter().copied()).collect()
    }

    #[test]
    fn parse_mjpeg_640_480_three_fps() {
        let extra = concat(&[&MJPEG_FORMAT, &MJPEG_FRAME_640_480]);
        let formats = parse_video_streaming_formats(&extra);
        assert_eq!(formats.len(), 3);
        let fps: Vec<u32> = formats.iter().map(CameraFormat::frame_rate).collect();
        assert_eq!(fps, vec![30, 15, 5]);
        for f in &formats {
            assert_eq!(f.format(), FrameFormat::MJPEG);
            assert_eq!(f.width(), 640);
            assert_eq!(f.height(), 480);
        }
    }

    #[test]
    fn parse_mjpeg_plus_nv12() {
        let extra = concat(&[
            &MJPEG_FORMAT,
            &MJPEG_FRAME_640_480,
            &NV12_FORMAT,
            &NV12_FRAME_320_240,
        ]);
        let formats = parse_video_streaming_formats(&extra);
        assert_eq!(formats.len(), 4, "3 MJPEG + 1 NV12 expected");
        assert_eq!(formats[3].format(), FrameFormat::NV12);
        assert_eq!(formats[3].width(), 320);
        assert_eq!(formats[3].height(), 240);
        assert_eq!(formats[3].frame_rate(), 30);
    }

    #[test]
    fn frame_without_preceding_format_is_skipped() {
        // A VS_FRAME without a preceding VS_FORMAT should be ignored
        // rather than emitted with a fabricated format.
        let formats = parse_video_streaming_formats(&MJPEG_FRAME_640_480);
        assert!(formats.is_empty());
    }

    #[test]
    fn unknown_uncompressed_guid_is_skipped() {
        // Same layout as NV12 but with a GUID we don't recognise.
        let mut fmt = NV12_FORMAT;
        fmt[5] = b'X';
        fmt[6] = b'X';
        fmt[7] = b'X';
        fmt[8] = b'X';
        let extra = concat(&[&fmt, &NV12_FRAME_320_240]);
        let formats = parse_video_streaming_formats(&extra);
        assert!(
            formats.is_empty(),
            "expected no formats for unknown GUID, got {formats:?}"
        );
    }

    #[test]
    fn truncated_descriptor_chain_does_not_loop() {
        // A zero-length descriptor would spin forever without the
        // `len < 3` guard.
        let mut extra = concat(&[&MJPEG_FORMAT, &MJPEG_FRAME_640_480]);
        extra.extend_from_slice(&[0x00, 0x24, 0x07]);
        let formats = parse_video_streaming_formats(&extra);
        // The valid prefix still produces its formats; the malformed
        // tail is skipped cleanly.
        assert_eq!(formats.len(), 3);
    }

    #[test]
    fn duplicate_tuples_are_deduped() {
        let extra = concat(&[
            &MJPEG_FORMAT,
            &MJPEG_FRAME_640_480,
            &MJPEG_FORMAT, // same format again
            &MJPEG_FRAME_640_480,
        ]);
        let formats = parse_video_streaming_formats(&extra);
        assert_eq!(
            formats.len(),
            3,
            "dedupe should collapse identical tuples: {formats:?}"
        );
    }

    #[test]
    fn interval_to_fps_rounds_correctly() {
        assert_eq!(interval_to_fps(333_333), Some(30));
        assert_eq!(interval_to_fps(166_666), Some(60));
        assert_eq!(interval_to_fps(2_000_000), Some(5));
        assert_eq!(interval_to_fps(0), None);
    }
}
