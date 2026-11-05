//! Write the still aurora to a PPM image, to check it by eye.
//! Usage: cargo run -p prism-ui --example aurora_png -- out.ppm [width height]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map_or("aurora.ppm", String::as_str);
    let w = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(160);
    let h = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(100);
    let img = prism_ui::aurora::render(w, h, 0.0);
    let mut out = format!("P6 {w} {h} 255\n").into_bytes();
    for p in &img.pixels {
        out.extend([p.r(), p.g(), p.b()]);
    }
    std::fs::write(path, out).expect("write");
}
