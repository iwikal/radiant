use radiant::RGB;

#[test]
fn old_decrunch_trivial() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 1\n\xff\x00\xff\x80";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(
        &image.data,
        &[RGB {
            r: 1.0,
            g: 0.0,
            b: 1.0,
        },]
    );
}

#[test]
fn old_decrunch_rle() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 2\n\xff\x00\xff\x80\x01\x01\x01\x01";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 2);
    assert_eq!(image.height, 1);
    assert_eq!(
        &image.data,
        &[
            RGB {
                r: 1.0,
                g: 0.0,
                b: 1.0,
            },
            RGB {
                r: 1.0,
                g: 0.0,
                b: 1.0,
            },
        ]
    );
}

#[test]
fn old_decrunch_rle_two_scanlines() {
    let reader = b"#?RADIANCE\0\n\n-Y 2 +X 2\n\
                 \xff\x00\xff\x80\x01\x01\x01\x01\
                 \x00\xff\x00\x80\x01\x01\x01\x01";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 2);
    assert_eq!(image.height, 2);
    assert_eq!(
        &image.data,
        &[
            RGB {
                r: 1.0,
                g: 0.0,
                b: 1.0,
            },
            RGB {
                r: 1.0,
                g: 0.0,
                b: 1.0,
            },
            RGB {
                r: 0.0,
                g: 1.0,
                b: 0.0,
            },
            RGB {
                r: 0.0,
                g: 1.0,
                b: 0.0,
            },
        ]
    );
}

#[test]
fn old_decrunch_zero_length_run() {
    let reader = b"#?RADIANCE\0\n\n-Y 1 +X 1\n\xff\x00\xff\x80\x01\x01\x01\x00";
    let image = radiant::load(&reader[..]).unwrap();
    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(
        &image.data,
        &[RGB {
            r: 1.0,
            g: 0.0,
            b: 1.0,
        },]
    );
}
