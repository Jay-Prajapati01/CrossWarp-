//! Local client for the optional IddCx frame bridge.
//!
//! This exports pixels from the driver only after `set_capture(true)` is called.
//! The caller is responsible for obtaining visible user consent before enabling
//! capture and for forwarding frames only through the authenticated session.

use std::ffi::c_void;
use std::ptr::NonNull;

const MAX_WIDTH: u32 = 3840;
const MAX_HEIGHT: u32 = 2160;
const MAX_FRAME_BYTES: usize = 32 + MAX_WIDTH as usize * MAX_HEIGHT as usize * 4;
const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;
const ERROR_NO_MORE_ITEMS: i32 = 259;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IddFrameInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: u32,
    pub sequence: u64,
    pub captured_at_millis: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum IddDisplayError {
    #[error("CrossWarp display driver interface is unavailable (Windows error {0})")]
    Open(i32),
    #[error("display capture request failed (Windows error {0})")]
    Request(i32),
    #[error("display driver returned malformed frame metadata")]
    InvalidFrame,
}

pub struct IddDisplayClient {
    handle: NonNull<c_void>,
    output: Vec<u8>,
    last_sequence: u64,
}

pub struct IddFrame<'a> {
    pub info: IddFrameInfo,
    pub bgra: &'a [u8],
}

unsafe extern "system" {
    fn cw_idd_client_open(client_out: *mut *mut c_void) -> i32;
    fn cw_idd_client_close(client: *mut c_void);
    fn cw_idd_client_set_capture(client: *mut c_void, enabled: u32) -> i32;
    fn cw_idd_client_read_frame(
        client: *mut c_void,
        after_sequence: u64,
        output: *mut c_void,
        output_length: u32,
        bytes_returned: *mut u32,
    ) -> i32;
}

impl IddDisplayClient {
    /// Open the single-client, local IDD interface. This does not enable capture.
    pub fn open() -> Result<Self, IddDisplayError> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: the FFI initializes `raw` on success and returns a Win32 error otherwise.
        let status = unsafe { cw_idd_client_open(&mut raw) };
        if status != 0 {
            return Err(IddDisplayError::Open(status));
        }
        let handle = NonNull::new(raw).ok_or(IddDisplayError::Open(6))?;
        Ok(Self {
            handle,
            output: vec![0; MAX_FRAME_BYTES],
            last_sequence: 0,
        })
    }

    /// Enable or disable IDD pixel capture after the caller obtains user consent.
    pub fn set_capture(&mut self, enabled: bool) -> Result<(), IddDisplayError> {
        // SAFETY: handle is owned by this client and the bridge accepts only 0 or 1.
        let status = unsafe { cw_idd_client_set_capture(self.handle.as_ptr(), u32::from(enabled)) };
        if status == 0 {
            if !enabled {
                self.last_sequence = 0;
            }
            Ok(())
        } else {
            Err(IddDisplayError::Request(status))
        }
    }

    /// Copy the newest frame newer than the last frame read. A caller can poll at
    /// its target cadence; old frames are skipped instead of queued.
    pub fn read_latest(&mut self) -> Result<Option<IddFrame<'_>>, IddDisplayError> {
        let mut bytes_returned = 0;
        // SAFETY: `output` is a bounded initialized buffer owned by this client.
        let status = unsafe {
            cw_idd_client_read_frame(
                self.handle.as_ptr(),
                self.last_sequence,
                self.output.as_mut_ptr().cast(),
                u32::try_from(self.output.len()).map_err(|_| IddDisplayError::InvalidFrame)?,
                &mut bytes_returned,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            return Ok(None);
        }
        if status != 0 {
            return Err(IddDisplayError::Request(status));
        }
        if bytes_returned < std::mem::size_of::<IddFrameInfo>() as u32 {
            return Err(IddDisplayError::InvalidFrame);
        }

        // The native ABI and Rust repr(C) header are kept in lockstep by tests.
        let info = unsafe { std::ptr::read_unaligned(self.output.as_ptr().cast::<IddFrameInfo>()) };
        if info.width == 0
            || info.height == 0
            || info.width > MAX_WIDTH
            || info.height > MAX_HEIGHT
            || info.stride != info.width.saturating_mul(4)
            || info.pixel_format != DXGI_FORMAT_B8G8R8A8_UNORM
            || info.sequence <= self.last_sequence
        {
            return Err(IddDisplayError::InvalidFrame);
        }
        let pixel_bytes = usize::try_from(info.stride)
            .ok()
            .and_then(|stride| stride.checked_mul(info.height as usize))
            .ok_or(IddDisplayError::InvalidFrame)?;
        let header_bytes = std::mem::size_of::<IddFrameInfo>();
        let expected_bytes = header_bytes
            .checked_add(pixel_bytes)
            .ok_or(IddDisplayError::InvalidFrame)?;
        if expected_bytes != bytes_returned as usize || expected_bytes > self.output.len() {
            return Err(IddDisplayError::InvalidFrame);
        }

        self.last_sequence = info.sequence;
        Ok(Some(IddFrame {
            info,
            bgra: &self.output[header_bytes..expected_bytes],
        }))
    }
}

impl Drop for IddDisplayClient {
    fn drop(&mut self) {
        // Closing the sole device handle disables the driver buffer and scrubs its last frame.
        unsafe { cw_idd_client_close(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_frame_abi_is_bounded_and_matches_the_idd_header() {
        assert_eq!(std::mem::size_of::<IddFrameInfo>(), 32);
        assert_eq!(MAX_FRAME_BYTES, 33_177_632);
        assert_eq!(DXGI_FORMAT_B8G8R8A8_UNORM, 87);
    }
}
