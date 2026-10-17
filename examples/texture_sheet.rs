//! Dev helper: renders every procedural texture (`materialcraft::proc_textures`) at its defaults to
//! out_dir/<family>.png, one contact sheet per family (each texture 256 px, two repeats in a row so seams
//! would show), and out_dir/<id>.png per texture. For judging the look-alikes by eye.
use materialcraft::proc_textures::{Family, TEXTURES, texture};
use materialcraft::tex::Tex;

const SIZE: usize = 256;

fn render(id: &str) -> Tex {
    let t = texture(id, &[]).unwrap();
    Tex::gray_fn(SIZE, SIZE, |u, v| t.value(u, v))
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "texture_sheet".into());
    std::fs::create_dir_all(&dir).unwrap();
    for family in [Family::Noise, Family::Grunge, Family::Spots, Family::Cells, Family::Pattern] {
        let members: Vec<_> = TEXTURES.iter().filter(|t| t.family == family).collect();
        let cols = 4usize;
        let rows = members.len().div_ceil(cols);
        // each cell shows the texture twice side by side (tile + its repeat)
        let (cw, ch) = (SIZE * 2 + 8, SIZE + 8);
        let mut sheet = Tex::solid(cols * cw, rows * ch, [0.2, 0.2, 0.25, 1.0]);
        for (i, info) in members.iter().enumerate() {
            let t = render(info.id);
            std::fs::write(format!("{dir}/{}.png", info.id), materialcraft::png::encode(t.w, t.h, &t.px, 8)).unwrap();
            let (ox, oy) = ((i % cols) * cw + 4, (i / cols) * ch + 4);
            for y in 0..SIZE {
                for x in 0..SIZE * 2 {
                    sheet.px[(oy + y) * sheet.w + ox + x] = t.px[y * SIZE + x % SIZE];
                }
            }
            println!("{:<10} {:<18} {}", family.label(), info.id, info.label);
        }
        std::fs::write(format!("{dir}/{}.png", family.label().to_lowercase()), materialcraft::png::encode(sheet.w, sheet.h, &sheet.px, 8)).unwrap();
    }
}
