//! Writes assets/icon.ico from the same drawing the window header uses: `cargo run --example make_icon`.

use std::io::Cursor;

use image::{ImageEncoder, codecs::png::PngEncoder};

fn main() {
    let sizes = [16usize, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&px| {
            let mut png = Vec::new();
            PngEncoder::new(Cursor::new(&mut png))
                .write_image(&nasdara_bunker_morse::logo::rasterize(px), px as u32, px as u32, image::ExtendedColorType::Rgba8)
                .expect("png");
            png
        })
        .collect();

    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (px, png) in sizes.iter().zip(&images) {
        let dim = if *px >= 256 { 0 } else { *px as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for png in &images {
        ico.extend_from_slice(png);
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon.ico");
    std::fs::write(&path, ico).expect("write icon");
    println!("wrote {}", path.display());
}
