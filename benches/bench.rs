#![feature(test)]

extern crate test;

use std::fs::File;
use std::io::BufReader;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    b.iter(|| {
        let f = File::open("assets/colorful_studio_2k.hdr").unwrap();
        let f = BufReader::new(f);
        radiant::load(f).unwrap()
    });
}
