use anyhow::*;
use minifb::{Key, Window, WindowOptions};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    pub image_path: PathBuf,
}

fn map_channel(v: f32) -> u32 {
    const COEFF: f32 = 127f32;
    u32::min(255, (2f32 * (v * COEFF)) as u32)
}

fn main() -> anyhow::Result<()> {
    let options = Options::from_args();
    let f = File::open(&options.image_path).context("Failed to open specified file")?;
    let f = BufReader::new(f);
    let image = radiant::load(f).context("Failed to load image data")?;

    let buf: Vec<_> = image
        .data
        .iter()
        .map(|px| {
            let r = map_channel(px.r);
            let g = map_channel(px.g);
            let b = map_channel(px.b);
            0xFF_00_00_00u32 | r << 16 | g << 8 | b
        })
        .collect();

    let width = image.width as usize;
    let height = image.height as usize;

    let title = format!("view_hdr: {}", options.image_path.to_string_lossy());
    let mut win = Window::new(&title, width, height, WindowOptions::default())
        .context("Failed to create window")?;

    win.update_with_buffer(buf.as_slice(), width, height)
        .context("Failed to render image")?;

    while win.is_open() && !win.is_key_down(Key::Escape) {
        win.update();
        sleep(Duration::from_millis(10));
    }

    Ok(())
}
