#![feature(test)]

extern crate test;

use test::Bencher;

#[bench]
fn full_load(b: &mut Bencher) {
    let f = &include_bytes!("../assets/colorful_studio_2k.hdr")[..];
    b.iter(|| radiant::load(f).unwrap());
}

#[bench]
fn scanlines(b: &mut Bencher) {
    let f = &include_bytes!("../assets/colorful_studio_2k.hdr")[..];

    b.iter(|| {
        const WIDTH: usize = 2048;
        const HEIGHT: usize = 1024;

        let mut buf = vec![radiant::Rgb::zero(); WIDTH];

        let mut loader = radiant::Loader::new(f).unwrap().scanlines();

        for _ in 0..HEIGHT {
            loader.read_scanline(&mut buf).unwrap();
        }

        buf
    });
}
