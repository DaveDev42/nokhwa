//! GStreamer backend controls (session 3).
//!
//! ## Scope
//!
//! GStreamer's source elements expose camera controls very unevenly:
//!
//! - **Linux `v4l2src`** — four `controllable` GObject properties
//!   (`brightness`, `contrast`, `hue`, `saturation`) readable + writable
//!   at any pipeline state. Other V4L2 controls (exposure, zoom, focus,
//!   pan/tilt, gain, sharpness, gamma, white-balance, backlight-comp)
//!   are reachable only through the write-only `extra-controls`
//!   `GstStructure`, which the element applies during the transition
//!   to PAUSED. We expose them via `set_control()`; reads for these
//!   controls are not supported through GStreamer — use the native
//!   `input-v4l` backend if full control introspection matters.
//! - **Windows `mfvideosrc` / `ksvideosrc`** — no camera-control
//!   properties whatsoever. `controls()` returns an empty list and
//!   `set_control()` errors. Users on Windows should use
//!   `input-msmf`.
//! - **macOS `avfvideosrc`** — treated the same as Windows until
//!   verified on real hardware.
//!
//! This module implements the Linux path; the others rely on the
//! introspection returning an empty list.

use gstreamer::glib::ParamFlags;
use gstreamer::prelude::*;
use gstreamer::Element;
use nokhwa_core::{
    error::NokhwaError,
    types::{
        ApiBackend, CameraControl, ControlValueDescription, ControlValueSetter, KnownCameraControl,
        KnownCameraControlFlag,
    },
};
use std::collections::BTreeMap;

/// GStreamer-side name for a control that we map to a [`KnownCameraControl`].
///
/// Two kinds: `Property(name)` is a direct `controllable` GObject
/// property on the source element (live read + write). `V4l2Cid(name)`
/// is a write-only entry delivered through the `extra-controls`
/// structure on Linux `v4l2src`.
#[derive(Copy, Clone, Debug)]
pub(crate) enum GstControlHandle {
    Property(&'static str),
    V4l2Cid(&'static str),
}

/// Static map from [`KnownCameraControl`] to the GStreamer name we
/// should use. Kept in one place so the V4L2-CID spelling and the
/// property spelling stay in sync. Returns `None` for
/// `KnownCameraControl::Other(_)` — callers are expected to pass raw
/// V4L2 CIDs via `set_control_extra` if they need something this map
/// doesn't know about.
pub(crate) fn control_handle(kcc: KnownCameraControl) -> Option<GstControlHandle> {
    Some(match kcc {
        KnownCameraControl::Brightness => GstControlHandle::Property("brightness"),
        KnownCameraControl::Contrast => GstControlHandle::Property("contrast"),
        KnownCameraControl::Hue => GstControlHandle::Property("hue"),
        KnownCameraControl::Saturation => GstControlHandle::Property("saturation"),
        KnownCameraControl::Sharpness => GstControlHandle::V4l2Cid("sharpness"),
        KnownCameraControl::Gamma => GstControlHandle::V4l2Cid("gamma"),
        KnownCameraControl::WhiteBalance => GstControlHandle::V4l2Cid("white_balance_temperature"),
        KnownCameraControl::BacklightComp => GstControlHandle::V4l2Cid("backlight_compensation"),
        KnownCameraControl::Gain => GstControlHandle::V4l2Cid("gain"),
        KnownCameraControl::Pan => GstControlHandle::V4l2Cid("pan_absolute"),
        KnownCameraControl::Tilt => GstControlHandle::V4l2Cid("tilt_absolute"),
        KnownCameraControl::Zoom => GstControlHandle::V4l2Cid("zoom_absolute"),
        KnownCameraControl::Exposure => GstControlHandle::V4l2Cid("exposure_time_absolute"),
        KnownCameraControl::Iris => GstControlHandle::V4l2Cid("iris_absolute"),
        KnownCameraControl::Focus => GstControlHandle::V4l2Cid("focus_absolute"),
        KnownCameraControl::Other(_) => return None,
    })
}

/// Enumerate the live, readable controls of `source`. Returns an empty
/// `Vec` on Windows / macOS because `mfvideosrc` / `ksvideosrc` /
/// `avfvideosrc` do not expose camera-control properties.
///
/// Uses `list_properties()` to discover which of the four `v4l2src`
/// properties the element actually offers, then reads the current
/// integer value and pspec range to build a [`CameraControl`].
pub(crate) fn list_controls(source: &Element) -> Vec<CameraControl> {
    let mut out = Vec::new();
    let pspecs = source.list_properties();
    for pspec in pspecs {
        let name = pspec.name();
        let Some(kcc) = known_from_property_name(name) else {
            continue;
        };
        // Filter out read-only / write-only properties — we need both
        // sides for a meaningful `CameraControl`.
        let flags = pspec.flags();
        if !flags.contains(ParamFlags::READABLE) || !flags.contains(ParamFlags::WRITABLE) {
            continue;
        }
        let Some(control) = build_integer_control(source, name, kcc, &pspec) else {
            continue;
        };
        out.push(control);
    }
    out
}

fn known_from_property_name(name: &str) -> Option<KnownCameraControl> {
    match name {
        "brightness" => Some(KnownCameraControl::Brightness),
        "contrast" => Some(KnownCameraControl::Contrast),
        "hue" => Some(KnownCameraControl::Hue),
        "saturation" => Some(KnownCameraControl::Saturation),
        _ => None,
    }
}

fn build_integer_control(
    source: &Element,
    name: &str,
    kcc: KnownCameraControl,
    pspec: &gstreamer::glib::ParamSpec,
) -> Option<CameraControl> {
    use gstreamer::glib::ParamSpecInt;

    let int_pspec = pspec.downcast_ref::<ParamSpecInt>()?;
    let value: i32 = source.property(name);
    Some(CameraControl::new(
        kcc,
        kcc.to_string(),
        ControlValueDescription::IntegerRange {
            min: i64::from(int_pspec.minimum()),
            max: i64::from(int_pspec.maximum()),
            value: i64::from(value),
            step: 1,
            default: i64::from(int_pspec.default_value()),
        },
        vec![KnownCameraControlFlag::Manual],
        true,
    ))
}

/// Apply a control whose [`GstControlHandle`] is a live GObject
/// property (`brightness` / `contrast` / `hue` / `saturation`). Works
/// at any pipeline state.
pub(crate) fn set_live_property(
    source: &Element,
    property: &str,
    value: &ControlValueSetter,
) -> Result<(), NokhwaError> {
    let int_value = match value {
        ControlValueSetter::Integer(i) => {
            i32::try_from(*i).map_err(|_| NokhwaError::SetPropertyError {
                property: property.to_string(),
                value: i.to_string(),
                error: "i64 value exceeds i32 range expected by v4l2src property".to_string(),
            })?
        }
        ControlValueSetter::Boolean(b) => i32::from(*b),
        other => {
            return Err(NokhwaError::SetPropertyError {
                property: property.to_string(),
                value: other.to_string(),
                error: "unsupported ControlValueSetter variant for live property".to_string(),
            });
        }
    };
    source.set_property(property, int_value);
    Ok(())
}

/// Build a GstStructure usable as `v4l2src`'s `extra-controls`
/// property from a pending-controls map. Returns `None` if the map is
/// empty so the caller can skip setting the property at all (empty
/// extra-controls is a no-op but triggers a warning).
pub(crate) fn build_extra_controls(
    pending: &BTreeMap<String, i64>,
) -> Option<gstreamer::Structure> {
    if pending.is_empty() {
        return None;
    }
    // v4l2src expects the structure name to be "c" (as in
    // `controls`); any other name is ignored silently.
    let mut builder = gstreamer::Structure::builder("c");
    for (k, v) in pending {
        builder = builder.field(k.as_str(), *v as i32);
    }
    Some(builder.build())
}

/// Translate a [`ControlValueSetter`] into the integer payload used by
/// V4L2 CIDs. Bool becomes 0/1.
pub(crate) fn v4l2_cid_value(cid: &str, value: &ControlValueSetter) -> Result<i64, NokhwaError> {
    match value {
        ControlValueSetter::Integer(i) => Ok(*i),
        ControlValueSetter::Boolean(b) => Ok(i64::from(*b)),
        other => Err(NokhwaError::SetPropertyError {
            property: cid.to_string(),
            value: other.to_string(),
            error: "unsupported ControlValueSetter variant for V4L2 CID".to_string(),
        }),
    }
}

/// Sentinel used by `set_control` when called on a platform where
/// GStreamer's source element has no usable control surface
/// (Windows / macOS today).
#[must_use]
pub(crate) fn unsupported() -> NokhwaError {
    NokhwaError::UnsupportedOperationError(ApiBackend::GStreamer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    /// `gstreamer::Structure::builder` requires the global registry to
    /// be initialised. Same `Once` guard pattern as `format::tests`.
    fn ensure_gst_init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            gstreamer::init().expect("gstreamer::init() must succeed in tests");
        });
    }

    #[test]
    fn control_handle_brightness_is_live_property() {
        match control_handle(KnownCameraControl::Brightness) {
            Some(GstControlHandle::Property("brightness")) => {}
            other => panic!("expected Property(\"brightness\"), got {other:?}"),
        }
    }

    #[test]
    fn control_handle_contrast_is_live_property() {
        match control_handle(KnownCameraControl::Contrast) {
            Some(GstControlHandle::Property("contrast")) => {}
            other => panic!("expected Property(\"contrast\"), got {other:?}"),
        }
    }

    #[test]
    fn control_handle_hue_is_live_property() {
        match control_handle(KnownCameraControl::Hue) {
            Some(GstControlHandle::Property("hue")) => {}
            other => panic!("expected Property(\"hue\"), got {other:?}"),
        }
    }

    #[test]
    fn control_handle_saturation_is_live_property() {
        match control_handle(KnownCameraControl::Saturation) {
            Some(GstControlHandle::Property("saturation")) => {}
            other => panic!("expected Property(\"saturation\"), got {other:?}"),
        }
    }

    #[test]
    fn control_handle_extra_controls_use_v4l2_cid_names() {
        // The `extra-controls` payload is a `GstStructure` keyed by
        // V4L2 CID name strings — these spellings are part of the
        // ABI contract with `v4l2src` and silent renames will silently
        // break control writes on real hardware.
        let pairs: &[(KnownCameraControl, &str)] = &[
            (KnownCameraControl::Sharpness, "sharpness"),
            (KnownCameraControl::Gamma, "gamma"),
            (
                KnownCameraControl::WhiteBalance,
                "white_balance_temperature",
            ),
            (KnownCameraControl::BacklightComp, "backlight_compensation"),
            (KnownCameraControl::Gain, "gain"),
            (KnownCameraControl::Pan, "pan_absolute"),
            (KnownCameraControl::Tilt, "tilt_absolute"),
            (KnownCameraControl::Zoom, "zoom_absolute"),
            (KnownCameraControl::Exposure, "exposure_time_absolute"),
            (KnownCameraControl::Iris, "iris_absolute"),
            (KnownCameraControl::Focus, "focus_absolute"),
        ];
        for (kcc, expected) in pairs {
            match control_handle(*kcc) {
                Some(GstControlHandle::V4l2Cid(name)) => {
                    assert_eq!(name, *expected, "wrong CID name for {kcc:?}");
                }
                other => panic!("expected V4l2Cid for {kcc:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn control_handle_other_returns_none() {
        // `KnownCameraControl::Other(_)` is the catch-all that callers
        // must dispatch to `set_control_extra` themselves; the static
        // map deliberately doesn't know about it.
        assert!(control_handle(KnownCameraControl::Other(0xdead_beef)).is_none());
    }

    #[test]
    fn control_handle_covers_every_known_variant() {
        // Pin: every variant of `KnownCameraControl` except `Other` must
        // produce some handle. A new variant added without a `match` arm
        // will fail compilation here.
        use nokhwa_core::types::all_known_camera_controls;
        for kcc in all_known_camera_controls() {
            assert!(
                control_handle(kcc).is_some(),
                "control_handle({kcc:?}) returned None — missing match arm?"
            );
        }
    }

    #[test]
    fn known_from_property_name_maps_four_v4l2src_props() {
        assert_eq!(
            known_from_property_name("brightness"),
            Some(KnownCameraControl::Brightness)
        );
        assert_eq!(
            known_from_property_name("contrast"),
            Some(KnownCameraControl::Contrast)
        );
        assert_eq!(
            known_from_property_name("hue"),
            Some(KnownCameraControl::Hue)
        );
        assert_eq!(
            known_from_property_name("saturation"),
            Some(KnownCameraControl::Saturation)
        );
    }

    #[test]
    fn known_from_property_name_unknown_returns_none() {
        // `list_controls` skips structures whose property name doesn't
        // match — anything outside the four `v4l2src` names is "not a
        // known camera control" and must return None.
        assert!(known_from_property_name("name").is_none());
        assert!(known_from_property_name("zoom_absolute").is_none());
        assert!(known_from_property_name("").is_none());
        assert!(known_from_property_name("Brightness").is_none()); // case-sensitive
    }

    #[test]
    fn build_extra_controls_empty_returns_none() {
        // Empty pending → caller skips the property set entirely
        // (setting an empty `extra-controls` triggers a v4l2src warning
        // at PAUSED transition).
        assert!(build_extra_controls(&BTreeMap::new()).is_none());
    }

    #[test]
    fn build_extra_controls_uses_structure_name_c() {
        ensure_gst_init();
        let mut pending = BTreeMap::new();
        pending.insert("zoom_absolute".to_string(), 5);
        let structure = build_extra_controls(&pending).expect("Some for non-empty pending");
        // v4l2src ignores the structure if the name is anything other
        // than "c" — pin the spelling.
        assert_eq!(structure.name(), "c");
    }

    #[test]
    fn build_extra_controls_writes_field_per_entry() {
        ensure_gst_init();
        let mut pending = BTreeMap::new();
        pending.insert("zoom_absolute".to_string(), 5);
        pending.insert("focus_absolute".to_string(), 100);
        pending.insert("exposure_time_absolute".to_string(), 250);
        let structure = build_extra_controls(&pending).unwrap();
        assert_eq!(structure.get::<i32>("zoom_absolute").unwrap(), 5);
        assert_eq!(structure.get::<i32>("focus_absolute").unwrap(), 100);
        assert_eq!(structure.get::<i32>("exposure_time_absolute").unwrap(), 250);
    }

    #[test]
    fn build_extra_controls_truncates_i64_to_i32() {
        // V4L2 CIDs are `__s32`-valued; a caller passing an out-of-range
        // i64 today gets a silent `as i32` truncation. This is documented
        // (only) by this test — if/when we change the signature to
        // surface the overflow, update this test.
        ensure_gst_init();
        let mut pending = BTreeMap::new();
        pending.insert("focus_absolute".to_string(), i64::from(i32::MAX) + 1);
        let structure = build_extra_controls(&pending).unwrap();
        assert_eq!(structure.get::<i32>("focus_absolute").unwrap(), i32::MIN);
    }

    #[test]
    fn v4l2_cid_value_integer_passthrough() {
        let v = v4l2_cid_value("zoom_absolute", &ControlValueSetter::Integer(42)).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn v4l2_cid_value_boolean_maps_to_zero_or_one() {
        assert_eq!(
            v4l2_cid_value("backlight_compensation", &ControlValueSetter::Boolean(true)).unwrap(),
            1
        );
        assert_eq!(
            v4l2_cid_value(
                "backlight_compensation",
                &ControlValueSetter::Boolean(false)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn v4l2_cid_value_rejects_unsupported_setters() {
        // V4L2 CIDs are int-valued — Float / String / RGB etc. must
        // fail with a SetPropertyError that names the offending CID.
        let unsupported_setters = [
            ControlValueSetter::Float(1.5),
            ControlValueSetter::String("foo".into()),
            ControlValueSetter::Bytes(vec![1, 2, 3]),
            ControlValueSetter::KeyValue(1, 2),
            ControlValueSetter::Point(0.0, 0.0),
            ControlValueSetter::RGB(0.0, 0.0, 0.0),
            ControlValueSetter::None,
            ControlValueSetter::EnumValue(7),
        ];
        for setter in &unsupported_setters {
            let err = v4l2_cid_value("focus_absolute", setter).unwrap_err();
            let s = format!("{err}");
            assert!(
                s.contains("focus_absolute"),
                "error message must mention CID, got: {s}"
            );
        }
    }

    #[test]
    fn unsupported_returns_unsupported_operation_error() {
        // The Windows / macOS sentinel — must surface `GStreamer` so
        // the user knows which backend lacks the support, not just
        // "unsupported".
        let err = unsupported();
        let msg = format!("{err}");
        assert!(
            matches!(
                err,
                NokhwaError::UnsupportedOperationError(ApiBackend::GStreamer)
            ),
            "wrong variant: {err:?}"
        );
        assert!(msg.contains("GStreamer"), "Display lost backend: {msg}");
    }
}
