use structopt::StructOpt;
use std::path::PathBuf;
use std::fs::File;
use std::thread::sleep;
use std::time::Duration;
use minifb::{Window, WindowOptions, Key};

#[derive(Debug, StructOpt)]
struct Options {
    pub image_path: PathBuf,
}

fn map_channel(v: f32) -> u32 {
    const COEFF: f32 = 127f32;
    u32::min(255, (2f32 * (v * COEFF)) as u32)
}

fn main() {
    let options = Options::from_args();
    let f = File::open(&options.image_path).expect("Failed to open specified file");
    let image = hdrldr::load(f).expect("Failed to load image data");
    
    let buf: Vec<_> = image.data.iter().map(|px| { 
        let r = map_channel(px.r);
        let g = map_channel(px.g);
        let b = map_channel(px.b);
        0xFF_00_00_00u32 | r << 16 | g << 8 | b
    }).collect();

    let title = format!("view_hdr: {}", options.image_path.to_string_lossy());
    let mut win = Window::new(&title, image.width as usize, image.height as usize, WindowOptions::default())
        .expect("Failed to create window");
    
    win.update_with_buffer(buf.as_slice())
        .expect("Failed to render image");

    while win.is_open() && !win.is_key_down(Key::Escape) {
        win.update();
        sleep(Duration::from_millis(10));
    }
}
