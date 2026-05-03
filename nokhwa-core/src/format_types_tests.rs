use super::*;

#[test]
fn yuyv_marker_maps_to_frame_format_yuyv() {
    assert_eq!(Yuyv::FRAME_FORMAT, FrameFormat::YUYV);
}

#[test]
fn nv12_marker_maps_to_frame_format_nv12() {
    assert_eq!(Nv12::FRAME_FORMAT, FrameFormat::NV12);
}

#[test]
fn mjpeg_marker_maps_to_frame_format_mjpeg() {
    assert_eq!(Mjpeg::FRAME_FORMAT, FrameFormat::MJPEG);
}

#[test]
fn gray_marker_maps_to_frame_format_gray() {
    assert_eq!(Gray::FRAME_FORMAT, FrameFormat::GRAY);
}

#[test]
fn raw_rgb_marker_maps_to_frame_format_rawrgb() {
    assert_eq!(RawRgb::FRAME_FORMAT, FrameFormat::RAWRGB);
}

#[test]
fn raw_bgr_marker_maps_to_frame_format_rawbgr() {
    assert_eq!(RawBgr::FRAME_FORMAT, FrameFormat::RAWBGR);
}

#[test]
fn marker_constants_are_pairwise_distinct() {
    let consts = [
        Yuyv::FRAME_FORMAT,
        Nv12::FRAME_FORMAT,
        Mjpeg::FRAME_FORMAT,
        Gray::FRAME_FORMAT,
        RawRgb::FRAME_FORMAT,
        RawBgr::FRAME_FORMAT,
    ];
    let mut sorted = consts.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        consts.len(),
        "two markers map to the same FrameFormat: {consts:?}",
    );
}

#[test]
fn marker_zsts_are_zero_sized() {
    use std::mem::size_of;
    assert_eq!(size_of::<Yuyv>(), 0);
    assert_eq!(size_of::<Nv12>(), 0);
    assert_eq!(size_of::<Mjpeg>(), 0);
    assert_eq!(size_of::<Gray>(), 0);
    assert_eq!(size_of::<RawRgb>(), 0);
    assert_eq!(size_of::<RawBgr>(), 0);
}

// Pin the derive surface on every marker ZST. The types are zero-sized
// dispatch tags, but downstream code (and the typed-frame conversion
// traits in `frame.rs`) silently relies on them being `Copy + Default +
// Hash + Ord + Debug` — a regression that drops one of these derives
// (e.g. removing `Hash` because no in-tree consumer obviously needs it)
// would break a downstream `HashMap<Mjpeg, _>` or a hand-rolled
// `BTreeSet<RawRgb>` without any compile error here. Enforcing the
// derives via a `fn assert_traits<T: …>()` shim makes the contract a
// compile-time test: removing any derive fails this file to build.

fn assert_marker_traits<T>()
where
    T: Copy
        + Clone
        + Default
        + std::fmt::Debug
        + std::hash::Hash
        + Ord
        + PartialOrd
        + Eq
        + PartialEq
        + Send
        + Sync
        + 'static,
{
}

#[test]
fn marker_derives_full_trait_set() {
    assert_marker_traits::<Yuyv>();
    assert_marker_traits::<Nv12>();
    assert_marker_traits::<Mjpeg>();
    assert_marker_traits::<Gray>();
    assert_marker_traits::<RawRgb>();
    assert_marker_traits::<RawBgr>();
}

// `CaptureFormat: Send + Sync + 'static` is the super-trait bound that
// makes marker types embeddable in `Arc`-shared backend state. A
// future refactor that loosens the super-trait (e.g. dropping `Send`
// to allow non-thread-safe markers) would silently break every
// `Arc<Mutex<Frame<F>>>` downstream. Pin via a generic helper that
// only compiles when the bound holds.

fn assert_capture_format_super_traits<T: CaptureFormat>() {
    fn require_send_sync_static<U: Send + Sync + 'static>() {}
    require_send_sync_static::<T>();
}

#[test]
fn capture_format_super_traits_are_send_sync_static() {
    assert_capture_format_super_traits::<Yuyv>();
    assert_capture_format_super_traits::<Nv12>();
    assert_capture_format_super_traits::<Mjpeg>();
    assert_capture_format_super_traits::<Gray>();
    assert_capture_format_super_traits::<RawRgb>();
    assert_capture_format_super_traits::<RawBgr>();
}

// Default-constructed markers must round-trip through `Hash` /
// `BTreeSet` so that downstream hash/tree-keyed dispatch works. ZSTs
// have a single value; this tests that the derived `Hash` and `Ord`
// produce consistent output rather than (e.g.) panicking or returning
// different hashes per call. The `Mjpeg::default()` call below is
// deliberately verbose to exercise the `Default` derive — clippy's
// `default_constructed_unit_structs` lint would prefer `Mjpeg`, but
// that hides the actual contract under test.
#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn marker_default_is_hash_and_btree_keyable() {
    use std::collections::{BTreeSet, HashSet};
    let mut hs = HashSet::new();
    hs.insert(Yuyv);
    hs.insert(Yuyv);
    assert_eq!(hs.len(), 1, "Hash should treat ZST default as one value");

    let mut bs = BTreeSet::new();
    bs.insert(Mjpeg);
    bs.insert(Mjpeg::default());
    assert_eq!(bs.len(), 1, "Ord should treat ZST default as one value");
}
