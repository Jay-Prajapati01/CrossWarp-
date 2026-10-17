//! Safe wrapper around the consent-gated Win32 Raw Input and SendInput bridge.
//! Capture must only be started after the product's explicit input-sharing consent.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{self, Receiver, Sender};

const INPUT_QUEUE_CAPACITY: usize = 4096;
const UW_INPUT_FLAG_ABSOLUTE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct NativeEvent {
    kind: u32,
    code: u32,
    x: i32,
    y: i32,
    value: i32,
    flags: u32,
    scan_code: u32,
    message_time_ms: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKind {
    PointerMove,
    PointerButton,
    PointerWheel,
    Key,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputEvent {
    pub kind: InputKind,
    pub code: u32,
    pub x: i32,
    pub y: i32,
    /// Button/key: 1 means down and 0 means up. Wheel: signed delta.
    pub value: i32,
    pub flags: u32,
    pub scan_code: u32,
    pub message_time_ms: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum InputBridgeError {
    #[error("Win32 Raw Input capture could not start (error {0})")]
    CaptureStart(i32),
    #[error("the native bridge returned an invalid capture handle")]
    InvalidHandle,
    #[error("input event is invalid for SendInput")]
    InvalidEvent,
    #[error("Windows rejected or blocked SendInput; UIPI may apply")]
    InjectionRejected,
    #[error("native input queue overflowed; stop routing and release held inputs")]
    QueueOverflow,
}

type Callback = unsafe extern "system" fn(*const NativeEvent, *mut c_void);

unsafe extern "system" {
    fn uw_input_capture_start(
        callback: Option<Callback>,
        context: *mut c_void,
        capture_out: *mut *mut c_void,
    ) -> i32;
    fn uw_input_capture_stop(capture: *mut c_void);
    fn uw_input_inject(event: *const NativeEvent) -> i32;
}

/// Active native device capture. Dropping this value unregisters both devices.
pub struct InputCapture {
    handle: usize,
    _callback_context: Box<CaptureContext>,
    events: Receiver<InputEvent>,
}

struct CaptureContext {
    sender: Sender<InputEvent>,
    overflow: AtomicBool,
}

impl InputCapture {
    /// Starts system mouse/keyboard Raw Input capture. The application must show
    /// and receive user consent before calling this method.
    pub fn start_after_user_consent() -> Result<Self, InputBridgeError> {
        let (sender, events) = mpsc::channel(INPUT_QUEUE_CAPACITY);
        let mut callback_context = Box::new(CaptureContext {
            sender,
            overflow: AtomicBool::new(false),
        });
        let context = (&mut *callback_context as *mut CaptureContext).cast();
        let mut handle = std::ptr::null_mut();
        // SAFETY: callback_context remains alive until after native capture stops.
        let status = unsafe { uw_input_capture_start(Some(on_native_event), context, &mut handle) };
        if status != 0 {
            return Err(InputBridgeError::CaptureStart(status));
        }
        if handle.is_null() {
            return Err(InputBridgeError::InvalidHandle);
        }
        Ok(Self {
            handle: handle as usize,
            _callback_context: callback_context,
            events,
        })
    }

    pub async fn recv(&mut self) -> Result<Option<InputEvent>, InputBridgeError> {
        if self
            ._callback_context
            .overflow
            .swap(false, Ordering::AcqRel)
        {
            return Err(InputBridgeError::QueueOverflow);
        }
        Ok(self.events.recv().await)
    }
}

impl Drop for InputCapture {
    fn drop(&mut self) {
        // SAFETY: handle was created by the native bridge and is stopped once.
        unsafe { uw_input_capture_stop(self.handle as *mut c_void) };
    }
}

/// Injects an event without bypassing Windows integrity-level restrictions.
pub fn inject(event: InputEvent) -> Result<(), InputBridgeError> {
    let native = to_native(event)?;
    // SAFETY: `native` is a valid repr(C) event for the duration of the call.
    match unsafe { uw_input_inject(&native) } {
        1 => Ok(()),
        0 => Err(InputBridgeError::InjectionRejected),
        _ => Err(InputBridgeError::InvalidEvent),
    }
}

unsafe extern "system" fn on_native_event(event: *const NativeEvent, context: *mut c_void) {
    if event.is_null() || context.is_null() {
        return;
    }
    // SAFETY: pointers are provided synchronously by the bridge; context points
    // to the sender owned by InputCapture, which outlives the capture thread.
    let native = unsafe { *event };
    let sender = unsafe { &*(context.cast::<CaptureContext>()) };
    if let Some(event) = from_native(native) {
        if sender.sender.try_send(event).is_err() {
            sender.overflow.store(true, Ordering::Release);
        }
    }
}

fn from_native(event: NativeEvent) -> Option<InputEvent> {
    let kind = match event.kind {
        1 => InputKind::PointerMove,
        2 => InputKind::PointerButton,
        3 => InputKind::PointerWheel,
        4 => InputKind::Key,
        _ => return None,
    };
    Some(InputEvent {
        kind,
        code: event.code,
        x: event.x,
        y: event.y,
        value: event.value,
        flags: event.flags,
        scan_code: event.scan_code,
        message_time_ms: event.message_time_ms,
    })
}

fn to_native(event: InputEvent) -> Result<NativeEvent, InputBridgeError> {
    let kind = match event.kind {
        InputKind::PointerMove => 1,
        InputKind::PointerButton => 2,
        InputKind::PointerWheel => 3,
        InputKind::Key => 4,
    };
    if event.kind == InputKind::PointerButton && !(1..=5).contains(&event.code)
        || event.kind == InputKind::Key && event.code > 0xFF
        || event.kind == InputKind::PointerMove
            && event.flags & UW_INPUT_FLAG_ABSOLUTE != 0
            && !(0..=65_535).contains(&event.x)
        || event.kind == InputKind::PointerMove
            && event.flags & UW_INPUT_FLAG_ABSOLUTE != 0
            && !(0..=65_535).contains(&event.y)
        || (event.kind == InputKind::PointerButton || event.kind == InputKind::Key)
            && !matches!(event.value, 0 | 1)
    {
        return Err(InputBridgeError::InvalidEvent);
    }
    Ok(NativeEvent {
        kind,
        code: event.code,
        x: event.x,
        y: event.y,
        value: event.value,
        flags: event.flags,
        scan_code: event.scan_code,
        message_time_ms: event.message_time_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_injection_rejects_invalid_transitions_before_calling_windows() {
        assert!(matches!(
            to_native(InputEvent {
                kind: InputKind::PointerButton,
                code: 6,
                x: 0,
                y: 0,
                value: 1,
                flags: 0,
                scan_code: 0,
                message_time_ms: 0
            }),
            Err(InputBridgeError::InvalidEvent)
        ));
        assert!(matches!(
            to_native(InputEvent {
                kind: InputKind::Key,
                code: 0x100,
                x: 0,
                y: 0,
                value: 1,
                flags: 0,
                scan_code: 0,
                message_time_ms: 0
            }),
            Err(InputBridgeError::InvalidEvent)
        ));
        assert!(matches!(
            to_native(InputEvent {
                kind: InputKind::Key,
                code: 0x41,
                x: 0,
                y: 0,
                value: 4,
                flags: 0,
                scan_code: 0,
                message_time_ms: 0
            }),
            Err(InputBridgeError::InvalidEvent)
        ));
        assert!(matches!(
            to_native(InputEvent {
                kind: InputKind::PointerMove,
                code: 0,
                x: 65_536,
                y: 0,
                value: 0,
                flags: UW_INPUT_FLAG_ABSOLUTE,
                scan_code: 0,
                message_time_ms: 0
            }),
            Err(InputBridgeError::InvalidEvent)
        ));
        assert!(to_native(InputEvent {
            kind: InputKind::PointerWheel,
            code: 0,
            x: 0,
            y: 0,
            value: 120,
            flags: 0,
            scan_code: 0,
            message_time_ms: 0
        })
        .is_ok());
    }
}
