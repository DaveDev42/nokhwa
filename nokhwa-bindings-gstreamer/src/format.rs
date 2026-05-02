//! Caps ↔ [`CameraFormat`] translation.
//!
//! GStreamer represents supported formats as a `Caps` structure with a
//! `video/x-raw` media type and a handful of well-known fields
//! (`format`, `width`, `height`, `framerate`). We normalise those into
//! nokhwa's `CameraFormat` tuples so the rest of the crate can speak
//! the same vocabulary as the other backends.
//!
//! Session 2 scope: uncompressed `video/x-raw` only. MJPEG via
//! `image/jpeg` caps is deferred — the native backends already cover
//! MJPEG-capable devices, so there's no user-facing regression until a
//! future session adds the compressed path here.

use gstreamer::Caps;
use gstreamer_video::VideoFormat;
use nokhwa_core::types::{CameraFormat, FrameFormat, Resolution};

/// Map GStreamer's [`VideoFormat`] to nokhwa's [`FrameFormat`]. Returns
/// `None` for formats we don't currently wire through (e.g. I420 /
/// RGB / BGRx) — those callers treat as "skip this caps structure",
/// same pattern MSMF uses for unknown subtypes.
pub(crate) fn video_format_to_frame_format(fmt: VideoFormat) -> Option<FrameFormat> {
    match fmt {
        VideoFormat::Yuy2 => Some(FrameFormat::YUYV),
        VideoFormat::Nv12 => Some(FrameFormat::NV12),
        VideoFormat::Gray8 => Some(FrameFormat::GRAY),
        _ => None,
    }
}

/// Inverse of [`video_format_to_frame_format`]. `None` for formats
/// nokhwa exposes but GStreamer doesn't round-trip (currently only
/// `MJPEG` and `RAWBGR` / `RAWRGB`, none of which are in the session-2
/// happy path).
pub(crate) fn frame_format_to_video_format(fmt: FrameFormat) -> Option<VideoFormat> {
    match fmt {
        FrameFormat::YUYV => Some(VideoFormat::Yuy2),
        FrameFormat::NV12 => Some(VideoFormat::Nv12),
        FrameFormat::GRAY => Some(VideoFormat::Gray8),
        _ => None,
    }
}

/// Expand a device's capability list into a flat `Vec<CameraFormat>`.
///
/// One `Caps` structure can carry a *list* of framerates and a *range*
/// of widths/heights; we explode the list side but keep ranges folded
/// to the declared `width`/`height` values. Any structure whose
/// `format`, `width`, `height`, or framerate list we cannot interpret
/// is skipped silently — same behaviour as MSMF's
/// `parse_native_media_types` when it hits a GUID it doesn't know.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn caps_to_camera_formats(caps: &Caps) -> Vec<CameraFormat> {
    let mut out = Vec::new();
    for i in 0..caps.size() {
        let Some(structure) = caps.structure(i) else {
            continue;
        };
        // Session 2: uncompressed video/x-raw only.
        if structure.name() != "video/x-raw" {
            continue;
        }
        let Ok(format_name) = structure.get::<&str>("format") else {
            continue;
        };
        let video_format = VideoFormat::from_string(format_name);
        let Some(frame_format) = video_format_to_frame_format(video_format) else {
            continue;
        };
        let Ok(width) = structure.get::<i32>("width") else {
            continue;
        };
        let Ok(height) = structure.get::<i32>("height") else {
            continue;
        };
        if width <= 0 || height <= 0 {
            continue;
        }
        let resolution = Resolution::new(width as u32, height as u32);

        // `framerate` can be either a single `Fraction` or a
        // `FractionList`. Try both.
        let rates = collect_framerates(structure);
        for fps in rates {
            out.push(CameraFormat::new(resolution, frame_format, fps));
        }
    }
    dedupe(out)
}

fn collect_framerates(structure: &gstreamer::structure::StructureRef) -> Vec<u32> {
    use gstreamer::{Fraction, FractionRange};

    if let Ok(single) = structure.get::<Fraction>("framerate") {
        return fraction_to_fps(single).into_iter().collect();
    }
    if let Ok(list) = structure.get::<gstreamer::List>("framerate") {
        let mut rates = Vec::new();
        for v in list.iter() {
            if let Ok(frac) = v.get::<Fraction>() {
                if let Some(fps) = fraction_to_fps(frac) {
                    rates.push(fps);
                }
            }
        }
        return rates;
    }
    // `FractionRange` is how Windows `mfvideosrc` (and some other
    // sources) advertise supported framerates — e.g. `[5/1, 60/1]`
    // means "any integer fps between 5 and 60 inclusive." We enumerate
    // a curated set of common rates that fall within the range rather
    // than exposing every integer, which would explode
    // `compatible_formats()` into hundreds of near-duplicates.
    if let Ok(range) = structure.get::<FractionRange>("framerate") {
        return enumerate_range(range);
    }
    // Silly ranges like `videotestsrc`'s `[0/1, 2147483647/1]` fall
    // through to an empty vec — we don't want to pretend those are
    // real options.
    Vec::new()
}

/// Common user-facing framerates that fall within a GStreamer
/// [`FractionRange`]. Keeps `compatible_formats()` lists tractable
/// (Windows mfvideosrc advertises 5–60 as a range, which would be 56
/// entries per resolution if we emitted every integer).
fn enumerate_range(range: gstreamer::FractionRange) -> Vec<u32> {
    const COMMON_FPS: &[u32] = &[5, 10, 15, 20, 24, 25, 30, 48, 50, 60, 90, 100, 120];
    let Some(min) = fraction_to_fps(range.min()) else {
        return Vec::new();
    };
    let Some(max) = fraction_to_fps(range.max()) else {
        return Vec::new();
    };
    if min > max {
        return Vec::new();
    }
    COMMON_FPS
        .iter()
        .copied()
        .filter(|fps| *fps >= min && *fps <= max)
        .collect()
}

/// Reject non-integer framerates — they are lossy for `CameraFormat`'s
/// `u32 fps` field. `gstreamer::Fraction`'s own invariant guarantees
/// a positive denominator (the constructor panics on 0), so we only
/// need to gate on the numerator and the integer-division remainder.
fn fraction_to_fps(frac: gstreamer::Fraction) -> Option<u32> {
    let num = frac.numer();
    let den = frac.denom();
    if num <= 0 {
        return None;
    }
    if num % den != 0 {
        return None;
    }
    u32::try_from(num / den).ok()
}

fn dedupe(mut v: Vec<CameraFormat>) -> Vec<CameraFormat> {
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yuy2_maps_to_yuyv() {
        assert_eq!(
            video_format_to_frame_format(VideoFormat::Yuy2),
            Some(FrameFormat::YUYV)
        );
    }

    #[test]
    fn nv12_round_trips() {
        let rt =
            frame_format_to_video_format(FrameFormat::NV12).and_then(video_format_to_frame_format);
        assert_eq!(rt, Some(FrameFormat::NV12));
    }

    #[test]
    fn unknown_video_format_returns_none() {
        assert_eq!(video_format_to_frame_format(VideoFormat::I420), None);
    }

    #[test]
    fn non_integer_framerate_rejected() {
        assert_eq!(
            fraction_to_fps(gstreamer::Fraction::new(30_000, 1001)),
            None
        );
    }

    #[test]
    fn integer_30fps_accepted() {
        assert_eq!(fraction_to_fps(gstreamer::Fraction::new(30, 1)), Some(30));
    }

    #[test]
    fn range_5_to_60_enumerates_common_rates() {
        // Matches Windows `mfvideosrc`'s `[5/1, 60/1]` advertisement.
        let range = gstreamer::FractionRange::new(
            gstreamer::Fraction::new(5, 1),
            gstreamer::Fraction::new(60, 1),
        );
        let rates = enumerate_range(range);
        assert!(rates.contains(&30), "should include 30fps: {rates:?}");
        assert!(rates.contains(&60), "should include 60fps: {rates:?}");
        assert!(!rates.contains(&120), "should exclude 120fps: {rates:?}");
    }

    #[test]
    fn absurd_range_returns_empty() {
        // `videotestsrc` advertises `[0/1, 2147483647/1]` — we'd
        // rather return nothing than expose garbage options.
        let range = gstreamer::FractionRange::new(
            gstreamer::Fraction::new(0, 1),
            gstreamer::Fraction::new(i32::MAX, 1),
        );
        // min=0 → fraction_to_fps returns None → empty.
        assert_eq!(enumerate_range(range), Vec::<u32>::new());
    }
}
