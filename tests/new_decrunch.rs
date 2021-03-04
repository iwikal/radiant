use radiant::Rgb;
use std::io::Read;

#[test]
fn new_decrunch_rle() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 8\n\
        \x02\x02\x08\x00\
        \x88\xff\x88\x00\x88\xff\x88\x80";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 8);
    assert_eq!(image.height, 1);
    assert_eq!(
        &image.data,
        &[Rgb {
            r: 1.0,
            g: 0.0,
            b: 1.0,
        }; 8]
    );
}

#[test]
fn new_decrunch_zero_length_run() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 8\n\
        \x02\x02\x08\x00\
        \x88\xff\x88\x00\x88\xff\x88\x80\x80\x56";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 8);
    assert_eq!(image.height, 1);
    assert_eq!(
        &image.data,
        &[Rgb {
            r: 1.0,
            g: 0.0,
            b: 1.0,
        }; 8]
    );
}

#[test]
fn new_decrunch_ignore_rest() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 8\n\
        \x02\x02\x08\x00\
        \x88\xff\x88\x00\x88\xff\x88\x80";
    let mut reader = reader.chain(&reader[..]);
    radiant::load(&mut reader).unwrap();
    radiant::load(&mut reader).unwrap();
}
