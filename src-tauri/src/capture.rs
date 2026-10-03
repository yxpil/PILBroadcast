// Screen capture via xcap + ring buffer for polling
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_FRAMES: usize = 10; // keep ~0.3 seconds at 30fps

#[derive(Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub seq: u64,
    #[allow(dead_code)]
    pub width: u32,
    #[allow(dead_code)]
    pub height: u32,
}

pub struct CaptureState {
    pub active: Arc<AtomicBool>,
    pub quality: Arc<Mutex<u8>>,
    pub fps: Arc<Mutex<u32>>,
    /// Ring buffer of recent frames — newest at back
    pub frames: Arc<Mutex<VecDeque<Frame>>>,
    /// Sequence counter
    pub seq: Arc<AtomicU64>,
    pub monitor_index: Arc<Mutex<u32>>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            quality: Arc::new(Mutex::new(60)),
            fps: Arc::new(Mutex::new(30)),
            frames: Arc::new(Mutex::new(VecDeque::new())),
            seq: Arc::new(AtomicU64::new(0)),
            monitor_index: Arc::new(Mutex::new(0)),
        }
    }
}

pub fn start_capture(state: &CaptureState) -> Result<(), String> {
    if state.active.load(Ordering::SeqCst) {
        return Err("Capture is already running".into());
    }
    state.active.store(true, Ordering::SeqCst);

    let active = Arc::clone(&state.active);
    let quality = Arc::clone(&state.quality);
    let fps = Arc::clone(&state.fps);
    let frames = Arc::clone(&state.frames);
    let seq = Arc::clone(&state.seq);
    let monitor = *state.monitor_index.lock().unwrap();

    thread::spawn(move || {
        capture_loop(active, quality, fps, frames, seq, monitor);
    });

    Ok(())
}

pub fn stop_capture(state: &CaptureState) {
    state.active.store(false, Ordering::SeqCst);
}

/// Get the next frame after `after_seq`. Returns (seq, base64 JPEG data).
pub fn poll_frame(frames: &Arc<Mutex<VecDeque<Frame>>>, after_seq: u64) -> Option<(u64, String)> {
    use base64::Engine;
    let queue = frames.lock().unwrap();
    let frame = queue.iter().find(|f| f.seq > after_seq).cloned();
    drop(queue);
    frame.map(|f| {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&f.data);
        (f.seq, b64)
    })
}

fn capture_loop(
    active: Arc<AtomicBool>,
    quality: Arc<Mutex<u8>>,
    fps: Arc<Mutex<u32>>,
    frames: Arc<Mutex<VecDeque<Frame>>>,
    seq: Arc<AtomicU64>,
    monitor_index: u32,
) {
    while active.load(Ordering::SeqCst) {
        let start = Instant::now();

        let monitors = xcap::Monitor::all().unwrap_or_default();
        if monitors.is_empty() {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let idx = (monitor_index as usize).min(monitors.len() - 1);
        if let Ok(img) = monitors[idx].capture_image() {
            let w = img.width();
            let h = img.height();
            let q = *quality.lock().unwrap();

            // xcap already converts BGRA→RGBA (bgra_to_rgba swaps B/R), so raw is [R,G,B,A]
            let raw = img.as_raw();
            let mut rgb = Vec::with_capacity((w * h * 3) as usize);
            for i in 0..(w * h) as usize {
                let idx = i * 4;
                rgb.push(raw[idx]);      // R
                rgb.push(raw[idx + 1]);  // G
                rgb.push(raw[idx + 2]);  // B
            }

            let mut buf = std::io::Cursor::new(Vec::new());
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q);
            if encoder.encode(&rgb, w, h, image::ExtendedColorType::Rgb8).is_ok() {
                let s = seq.fetch_add(1, Ordering::SeqCst);
                let frame = Frame {
                    data: buf.into_inner(),
                    seq: s,
                    width: w,
                    height: h,
                };

                let mut queue = frames.lock().unwrap();
                queue.push_back(frame);
                while queue.len() > MAX_FRAMES {
                    queue.pop_front();
                }
            }
        }

        let target_ms = 1000u64 / (*fps.lock().unwrap() as u64).max(1);
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed < target_ms {
            thread::sleep(Duration::from_millis(target_ms - elapsed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn frame(seq: u64, data: &[u8]) -> Frame {
        Frame { data: data.to_vec(), seq, width: 1, height: 1 }
    }

    #[test]
    fn default_state_is_sane() {
        let s = CaptureState::new();
        assert!(!s.active.load(Ordering::SeqCst), "fresh capture must be inactive");
        assert_eq!(*s.quality.lock().unwrap(), 60, "default quality is 60");
        assert_eq!(*s.fps.lock().unwrap(), 30, "default fps is 30");
        assert_eq!(s.frames.lock().unwrap().len(), 0, "queue starts empty");
        assert_eq!(s.seq.load(Ordering::SeqCst), 0, "seq starts at 0");
        assert_eq!(*s.monitor_index.lock().unwrap(), 0);
    }

    #[test]
    fn poll_frame_returns_strictly_next_seq() {
        let frames = Arc::new(Mutex::new(VecDeque::new()));
        {
            let mut q = frames.lock().unwrap();
            q.push_back(frame(1, b"a"));
            q.push_back(frame(2, b"b"));
            q.push_back(frame(3, b"c"));
        }
        // after_seq = 0 → first frame (page load gets oldest available)
        assert_eq!(poll_frame(&frames, 0).unwrap().0, 1);
        // incremental polling walks forward one frame at a time
        assert_eq!(poll_frame(&frames, 1).unwrap().0, 2);
        assert_eq!(poll_frame(&frames, 2).unwrap().0, 3);
        // nothing newer than the last frame
        assert!(poll_frame(&frames, 3).is_none());
        // a seq in the middle skips to the next one, never repeats
        assert_eq!(poll_frame(&frames, 1).unwrap().0, 2);
    }

    #[test]
    fn poll_frame_base64_encodes_payload() {
        let frames = Arc::new(Mutex::new(VecDeque::new()));
        frames.lock().unwrap().push_back(frame(7, b"hello"));
        let (seq, b64) = poll_frame(&frames, 0).expect("frame present");
        assert_eq!(seq, 7);
        assert_eq!(b64, base64::engine::general_purpose::STANDARD.encode(b"hello"));
    }

    #[test]
    fn poll_frame_empty_queue_is_none() {
        let frames = Arc::new(Mutex::new(VecDeque::new()));
        assert!(poll_frame(&frames, 0).is_none());
        assert!(poll_frame(&frames, 10_000).is_none());
    }

    #[test]
    fn start_capture_rejects_double_start() {
        let s = CaptureState::new();
        // set active without spawning a real capture thread (no display in CI)
        s.active.store(true, Ordering::SeqCst);
        let err = start_capture(&s).unwrap_err();
        assert!(err.contains("already running"), "unexpected error: {}", err);
    }

    #[test]
    fn stop_capture_clears_active_flag() {
        let s = CaptureState::new();
        s.active.store(true, Ordering::SeqCst);
        stop_capture(&s);
        assert!(!s.active.load(Ordering::SeqCst));
    }
}
