// xcap screenshot test
use std::fs;

fn main() {
    let monitors = xcap::Monitor::all().unwrap();
    println!("Monitors: {}", monitors.len());
    for m in &monitors {
        println!("  {}: {}x{} at ({},{}) scale={:.1}",
            m.name(), m.width(), m.height(), m.x(), m.y(), m.scale_factor());
    }

    let monitor = &monitors[0];
    let img = monitor.capture_image().unwrap();
    println!("Captured: {}x{} ({:.1} MB)", img.width(), img.height(), img.len() as f64 / 1_000_000.0);

    // xcap already converts BGRA→RGBA (bgra_to_rgba swaps B/R), so raw is [R,G,B,A]
    let w = img.width();
    let h = img.height();
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    let raw = img.as_raw();
    for i in 0..(w * h) as usize {
        let idx = i * 4;
        rgb.push(raw[idx]);      // R
        rgb.push(raw[idx + 1]);  // G
        rgb.push(raw[idx + 2]);  // B
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
    enc.encode(&rgb, w, h, image::ExtendedColorType::Rgb8).unwrap();

    let path = std::env::current_dir().unwrap().join("xcap_test.jpg");
    fs::write(&path, buf.into_inner()).unwrap();
    println!("OK: {}", path.display());
}
