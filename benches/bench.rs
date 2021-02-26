#![feature(test)]

extern crate test;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    let f = &include_bytes!("../assets/colorful_studio_2k.hdr")[..];
    b.iter(|| radiant::load(f).unwrap());
}
