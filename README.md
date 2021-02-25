<!-- cargo-sync-readme start -->

# Radiant

Load Radiance HDR (.hdr, .pic) images.

This is a fork of [TechPriest’s HdrLdr](https://crates.io/crates/hdrldr),
rewritten for slightly better performance. May or may not actually perform better.
I've restricted the API so that it only accepts readers that implement
`BufRead`.

The original crate, which does not have this restriction, is in turn a slightly
rustified version of [C++ code by IgorKravtchenko](http://flipcode.com/archives/HDR_Image_Reader.shtml). If you need
more image formats besides HDR, take a look at [Image2crate](https://crates.io/crates/image2).

## Example

Add `radiant` to your dependencies of your `Cargo.toml`:
```toml
[dependencies]
radiant = "0.2"
```

And then, in your rust file:
```rust
use std::io::BufReader;
use std::fs::File;

let f = File::open("assets/colorful_studio_2k.hdr").expect("Failed to open specified file");
let f = BufReader::new(f);
let image = radiant::load(f).expect("Failed to load image data");
```

For more complete example, see
[Simple HDR Viewer application](https://github.com/iwikal/radiant/blob/master/examples/view_hdr.rs)

Huge thanks to [HDRI Haven](https://hdrihaven.com) for providing CC0 sample images for testing!

<!-- cargo-sync-readme end -->
