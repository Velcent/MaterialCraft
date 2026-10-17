use materialcraft::{export, tex::Tex};
fn main() {
    let (w, h) = (130usize, 90usize);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let v = ((x as f32 / 9.0).sin() * 0.5 + 0.5) * (y as f32 / h as f32);
            px.push([x as f32 / w as f32, v, 1.0 - y as f32 / h as f32, if x < 20 && y < 20 { 0.4 } else { 1.0 }]);
        }
    }
    let t = Tex { w, h, px };
    std::fs::write(std::env::temp_dir().join("t.jpg"), export::encode(export::Format::Jpeg, &t, 8, 92)).unwrap();
    std::fs::write(std::env::temp_dir().join("t8.tif"), export::encode(export::Format::Tiff, &t, 8, 0)).unwrap();
    std::fs::write(std::env::temp_dir().join("t16.tif"), export::encode(export::Format::Tiff, &t, 16, 0)).unwrap();
    let mut o = Vec::new();
    for p in &t.px {
        o.extend_from_slice(&[(p[0] * 255.0 + 0.5) as u8, (p[1] * 255.0 + 0.5) as u8, (p[2] * 255.0 + 0.5) as u8]);
    }
    std::fs::write(std::env::temp_dir().join("t.raw"), o).unwrap();
}
