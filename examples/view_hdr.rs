

fn main() {

}


/*
#[test]

fn loads_hdr() {
    let data = include_bytes!("../test_data/cape_hill_1k.hdr");
    let r = std::io::Cursor::new(&data[..]);
    let img = load(r).unwrap();

    fn map_channel(v: f32) -> u32 {
        const COEFF: f32 = 127f32;
        u32::min(255, (2f32 * (v * COEFF)) as u32)
    }

    let buf: Vec<_> = img.data.iter().map(|px| { 
        let r = map_channel(px.r);
        let g = map_channel(px.g);
        let b = map_channel(px.b);
        0xFF_00_00_00u32 | r << 16 | g << 8 | b
    }).collect();

    let mut win = minifb::Window::new("cape_hill_1k", img.width as usize, img.height as usize, minifb::WindowOptions::default())
        .unwrap();

    while win.is_open() && !win.is_key_down(minifb::Key::Escape) {
        win.update_with_buffer(&buf);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
*/