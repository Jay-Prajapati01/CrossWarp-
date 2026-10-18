//! Windows.Graphics.Capture HWND source and native frame transport boundary.
//! Callers must obtain explicit source-side consent before starting a capture.

use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturableWindow {
    pub hwnd: u64,
    pub title: String,
}

pub struct CapturedFrame {
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    /// Top-down BGRA8 pixels.
    pub bgra: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct WindowCaptureError(String);

impl fmt::Display for WindowCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WindowCaptureError {}

type WindowCallback = unsafe extern "C" fn(u64, *const u16, usize, *mut c_void);

unsafe extern "C" {
    fn cw_capture_enumerate_windows(callback: Option<WindowCallback>, context: *mut c_void);
    fn cw_capture_start(hwnd: u64, capture_out: *mut *mut c_void) -> i32;
    fn cw_capture_read(
        capture: *mut c_void,
        after_sequence: u64,
        buffer: *mut u8,
        capacity: usize,
        required: *mut usize,
        width: *mut u32,
        height: *mut u32,
        sequence: *mut u64,
    ) -> i32;
    fn cw_capture_last_error() -> *const std::ffi::c_char;
    fn cw_capture_stop(capture: *mut c_void);
}

/// Lists visible top-level windows with titles, excluding windows owned by
/// this process. The returned HWND is an opaque local token, never sent over
/// the network.
pub fn enumerate_windows() -> Vec<CapturableWindow> {
    let mut windows = Vec::new();
    // SAFETY: the vector remains pinned at the same address for the synchronous
    // EnumWindows callback and each title is copied before callback return.
    unsafe {
        cw_capture_enumerate_windows(
            Some(collect_window),
            (&mut windows as *mut Vec<CapturableWindow>).cast(),
        );
    }
    windows
}

unsafe extern "C" fn collect_window(
    hwnd: u64,
    title: *const u16,
    title_length: usize,
    context: *mut c_void,
) {
    if title.is_null() || context.is_null() || title_length > 4096 {
        return;
    }
    let title = unsafe { std::slice::from_raw_parts(title, title_length) };
    let windows = unsafe { &mut *context.cast::<Vec<CapturableWindow>>() };
    windows.push(CapturableWindow {
        hwnd,
        title: String::from_utf16_lossy(title),
    });
}

/// Active per-window capture. The owning thread should poll read_latest and
/// drop this handle on that same thread.
pub struct WindowCapture {
    handle: NonNull<c_void>,
    last_sequence: u64,
}

/// Converts one native BGRA frame to a bounded JPEG suitable for the current
/// encrypted media record. Large sources are downscaled to at most 1280x720;
/// quality is reduced before returning an oversized payload.
pub fn encode_preview_jpeg(
    frame: &CapturedFrame,
) -> Result<(u32, u32, Vec<u8>), WindowCaptureError> {
    use image::imageops::FilterType;
    use image::{ImageBuffer, Rgba};
    use std::io::Cursor;

    if frame.width == 0 || frame.height == 0 {
        return Err(WindowCaptureError(
            "capture returned empty dimensions".into(),
        ));
    }
    let valid_length = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .is_some_and(|bytes| bytes == frame.bgra.len());
    if !valid_length {
        return Err(WindowCaptureError(
            "capture returned inconsistent BGRA data".into(),
        ));
    }
    let mut rgba = frame.bgra.clone();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let source = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(frame.width, frame.height, rgba)
        .ok_or_else(|| WindowCaptureError("capture pixels could not be decoded".into()))?;
    let scale = (1280.0f64 / f64::from(frame.width))
        .min(720.0f64 / f64::from(frame.height))
        .min(1.0);
    let width = (f64::from(frame.width) * scale).round().max(1.0) as u32;
    let height = (f64::from(frame.height) * scale).round().max(1.0) as u32;
    let resized = if width == frame.width && height == frame.height {
        source
    } else {
        image::imageops::resize(&source, width, height, FilterType::Triangle)
    };
    let rgb = image::DynamicImage::ImageRgba8(resized).into_rgb8();
    let mut last_size = 0usize;
    for quality in [78, 66, 54, 42] {
        let mut bytes = Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
            .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
            .map_err(|error| WindowCaptureError(format!("JPEG encoding failed: {error}")))?;
        last_size = bytes.get_ref().len();
        let payload = bytes.into_inner();
        if payload.len() <= crate::media_session::MAX_MEDIA_PAYLOAD_BYTES {
            return Ok((width, height, payload));
        }
    }
    Err(WindowCaptureError(format!(
        "JPEG frame remains too large after downscaling and quality reduction ({last_size} bytes)"
    )))
}

impl WindowCapture {
    pub fn start_after_user_consent(hwnd: u64) -> Result<Self, WindowCaptureError> {
        let mut handle = std::ptr::null_mut();
        // SAFETY: native start validates HWND and initializes the out pointer.
        let status = unsafe { cw_capture_start(hwnd, &mut handle) };
        if status != 0 {
            return Err(last_native_error(status));
        }
        let handle = NonNull::new(handle)
            .ok_or_else(|| WindowCaptureError("native capture returned a null handle".into()))?;
        Ok(Self {
            handle,
            last_sequence: 0,
        })
    }

    /// Returns only a fresh frame. A changed window size is reported with its
    /// actual dimensions; callers must resize and bound before encoding.
    pub fn read_latest(&mut self) -> Result<Option<CapturedFrame>, WindowCaptureError> {
        let mut required = 0usize;
        let mut width = 0u32;
        let mut height = 0u32;
        let mut sequence = 0u64;
        // SAFETY: the first call requests the required length without copying.
        let status = unsafe {
            cw_capture_read(
                self.handle.as_ptr(),
                self.last_sequence,
                std::ptr::null_mut(),
                0,
                &mut required,
                &mut width,
                &mut height,
                &mut sequence,
            )
        };
        if status == 0 {
            return Ok(None);
        }
        if status != -2 || required == 0 || required > 64 * 1024 * 1024 {
            return Err(last_native_error(status));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|bytes| *bytes == required)
            .ok_or_else(|| {
                WindowCaptureError("capture returned inconsistent frame dimensions".into())
            })?;
        let mut bgra = vec![0; expected];
        // SAFETY: bgra has the exact capacity reported by the native bridge.
        let copied = unsafe {
            cw_capture_read(
                self.handle.as_ptr(),
                self.last_sequence,
                bgra.as_mut_ptr(),
                bgra.len(),
                &mut required,
                &mut width,
                &mut height,
                &mut sequence,
            )
        };
        if copied != 1 {
            return Err(last_native_error(copied));
        }
        self.last_sequence = sequence;
        Ok(Some(CapturedFrame {
            sequence,
            width,
            height,
            bgra,
        }))
    }
}

impl Drop for WindowCapture {
    fn drop(&mut self) {
        // SAFETY: this handle is created by the native bridge and stopped once.
        unsafe { cw_capture_stop(self.handle.as_ptr()) };
    }
}

fn last_native_error(status: i32) -> WindowCaptureError {
    // SAFETY: the native function returns a thread-local nul-terminated string.
    let error = unsafe {
        let message = cw_capture_last_error();
        if message.is_null() {
            format!("window capture failed with status {status}")
        } else {
            std::ffi::CStr::from_ptr(message)
                .to_string_lossy()
                .into_owned()
        }
    };
    WindowCaptureError(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_window_handle_is_rejected_without_creating_a_capture() {
        assert!(WindowCapture::start_after_user_consent(0).is_err());
    }

    #[test]
    fn window_enumeration_returns_opaque_tokens_and_titles() {
        let windows = enumerate_windows();
        assert!(windows
            .iter()
            .all(|window| window.hwnd != 0 && !window.title.is_empty()));
    }

    #[test]
    fn preview_encoder_swizzles_bgra_and_returns_bounded_jpeg() {
        let frame = CapturedFrame {
            sequence: 1,
            width: 2,
            height: 2,
            // Four opaque blue BGRA pixels.
            bgra: vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        };
        let (width, height, jpeg) = encode_preview_jpeg(&frame).unwrap();
        assert_eq!((width, height), (2, 2));
        assert!(jpeg.starts_with(&[0xff, 0xd8]));
        assert!(jpeg.ends_with(&[0xff, 0xd9]));
        assert!(jpeg.len() <= crate::media_session::MAX_MEDIA_PAYLOAD_BYTES);
    }

    #[test]
    fn preview_encoder_downscales_large_window_frames_before_networking() {
        let width = 1920;
        let height = 1080;
        let frame = CapturedFrame {
            sequence: 2,
            width,
            height,
            bgra: vec![0; width as usize * height as usize * 4],
        };
        let (preview_width, preview_height, jpeg) = encode_preview_jpeg(&frame).unwrap();
        assert_eq!((preview_width, preview_height), (1280, 720));
        assert!(jpeg.len() <= crate::media_session::MAX_MEDIA_PAYLOAD_BYTES);
    }
}
