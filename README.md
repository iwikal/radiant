# HdrLdr

Load Radiance HDR (.hdr, .pic) images. Slightly rustified version of [C++ code by Igor Kravtchenko](http://flipcode.com/archives/HDR_Image_Reader.shtml). If you need more image formats besides HDR, take a look at [Image2 crate](https://crates.io/crates/image2). 

## Example

Add `hdrldr` to your dependencies of your `Cargo.toml`:
```toml
[dependencies]
hdrldr = "0.1"
```

And then, in your rust file:
```rust

fn main() {
    // ...
    let f = File::open("foo.hdr").expect("Failed to open specified file");
    let image = hdrldr::load(f).expect("Failed to load image data");
    // Use your image data
    // ...
}
```

For more complete example, see [Simple HDR Viewer application](https://github.com/TechPriest/hdrldr/blob/master/examples/view_hdr.rs)

