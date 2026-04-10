use crate::ffi::{
    kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs, kCMPixelFormat_8IndexedGray_WhiteIsZero,
    kCMVideoCodecType_422YpCbCr8, kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, OSType,
};
use flume::{Receiver, Sender};
use nokhwa_core::types::FrameFormat;
use objc2::runtime::AnyObject;
use std::{
    borrow::Cow,
    error::Error,
    ffi::{c_void, CStr},
};

pub(crate) const UTF8_ENCODING: usize = 4;

macro_rules! create_boilerplate_impl {
    {
        $( [$class_vis:vis $class_name:ident ] ),+
    } => {
        $(
            $class_vis struct $class_name {
                inner: *mut AnyObject,
            }

            impl $class_name {
                pub fn inner(&self) -> *mut AnyObject {
                    self.inner
                }
            }

            impl From<*mut AnyObject> for $class_name {
                fn from(obj: *mut AnyObject) -> Self {
                    $class_name {
                        inner: obj,
                    }
                }
            }
        )+
    };
}
pub(crate) use create_boilerplate_impl;

pub(crate) fn str_to_nsstr(string: &str) -> *mut AnyObject {
    let cls = objc2::class!(NSString);
    let bytes = string.as_ptr() as *const c_void;
    unsafe {
        let obj: *mut AnyObject = objc2::msg_send![cls, alloc];
        let obj: *mut AnyObject = objc2::msg_send![
            obj,
            initWithBytes:bytes,
            length:string.len(),
            encoding:UTF8_ENCODING
        ];
        obj
    }
}

pub(crate) fn nsstr_to_str<'a>(nsstr: *mut AnyObject) -> Cow<'a, str> {
    let utf8ptr: *const std::os::raw::c_char = unsafe { objc2::msg_send![nsstr, UTF8String] };
    let data = unsafe { CStr::from_ptr(utf8ptr) };
    data.to_string_lossy()
}

pub(crate) fn vec_to_ns_arr<T: Into<*mut AnyObject>>(data: Vec<T>) -> *mut AnyObject {
    let ns_arr_cls = objc2::class!(NSMutableArray);
    let mutable_array: *mut AnyObject = unsafe { objc2::msg_send![ns_arr_cls, array] };
    data.into_iter().for_each(|item| {
        let item_obj: *mut AnyObject = item.into();
        let _: () = unsafe { objc2::msg_send![mutable_array, addObject: item_obj] };
    });
    mutable_array
}

pub(crate) fn ns_arr_to_vec<T: From<*mut AnyObject>>(data: *mut AnyObject) -> Vec<T> {
    let length: usize = unsafe { objc2::msg_send![data, count] };

    let mut out_vec: Vec<T> = Vec::with_capacity(length);
    for index in 0..length {
        let item: *mut AnyObject = unsafe { objc2::msg_send![data, objectAtIndex: index] };
        out_vec.push(T::from(item));
    }
    out_vec
}

pub(crate) fn try_ns_arr_to_vec<T, TE>(data: *mut AnyObject) -> Result<Vec<T>, TE>
where
    TE: Error,
    T: TryFrom<*mut AnyObject, Error = TE>,
{
    let length: usize = unsafe { objc2::msg_send![data, count] };

    let mut out_vec: Vec<T> = Vec::with_capacity(length);
    for index in 0..length {
        let item: *mut AnyObject = unsafe { objc2::msg_send![data, objectAtIndex: index] };
        out_vec.push(T::try_from(item)?);
    }
    Ok(out_vec)
}

pub(crate) fn compare_ns_string(this: *mut AnyObject, other: crate::ffi::NSString) -> bool {
    unsafe {
        let equal: bool = objc2::msg_send![this, isEqualToString: other];
        equal
    }
}

#[allow(non_upper_case_globals)]
pub(crate) fn raw_fcc_to_frameformat(raw: OSType) -> Option<FrameFormat> {
    match raw {
        kCMVideoCodecType_422YpCbCr8 | kCMPixelFormat_422YpCbCr8_yuvs => Some(FrameFormat::YUYV),
        kCMVideoCodecType_JPEG | kCMVideoCodecType_JPEG_OpenDML => Some(FrameFormat::MJPEG),
        kCMPixelFormat_8IndexedGray_WhiteIsZero => Some(FrameFormat::GRAY),
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
        | kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange => Some(FrameFormat::NV12),
        kCMPixelFormat_24RGB => Some(FrameFormat::RAWRGB),
        _ => None,
    }
}

pub type CompressionData<'a> = (Cow<'a, [u8]>, FrameFormat, Option<std::time::Duration>);
pub type DataPipe<'a> = (Sender<CompressionData<'a>>, Receiver<CompressionData<'a>>);
