use std::time::{Duration, Instant};
use unified_workspace_agent::windows_capture::{encode_preview_jpeg, WindowCapture};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = std::env::args()
        .nth(1)
        .ok_or("usage: cargo run --example capture_probe -- <visible-window-HWND>")?;
    let hwnd = raw.trim_start_matches("0x");
    let hwnd = u64::from_str_radix(hwnd, 16)?;
    let mut capture = WindowCapture::start_after_user_consent(hwnd)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let frame = loop {
        if let Some(frame) = capture.read_latest()? {
            break frame;
        }
        if Instant::now() >= deadline {
            return Err("no frame arrived within five seconds".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let (width, height, jpeg) = encode_preview_jpeg(&frame)?;
    let digest = hex::encode(unified_workspace_agent::file_rpc::sha256(&jpeg));
    println!(
        "CAPTURED hwnd=0x{hwnd:X} sequence={} source={}x{} preview={}x{} jpeg_bytes={} sha256={digest}",
        frame.sequence,
        frame.width,
        frame.height,
        width,
        height,
        jpeg.len()
    );
    Ok(())
}
