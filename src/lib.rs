#![warn(missing_docs)]

//! # Radiant
//!
//! Load Radiance HDR (.hdr, .pic) images.
//!
//! This is a fork of [TechPriest's HdrLdr](https://crates.io/crates/hdrldr),
//! rewritten for slightly better performance. May or may not actually perform better.
//! I've restricted the API so that it only accepts readers that implement
//! `BufRead`.
//!
//! The original crate, which does not have this restriction, is in turn a slightly
//! rustified version of [C++ code by Igor
//! Kravtchenko](http://flipcode.com/archives/HDR_Image_Reader.shtml). If you need
//! more image formats besides HDR, take a look at [Image2
//! crate](https://crates.io/crates/image2).
//!
//! ## Example
//!
//! Add `radiant` to your dependencies of your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! radiant = "0.2"
//! ```
//!
//! And then, in your rust file:
//! ```rust
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! let f = File::open("assets/colorful_studio_2k.hdr").expect("Failed to open specified file");
//! let f = BufReader::new(f);
//! let image = radiant::load(f).expect("Failed to load image data");
//! ```
//!
//! For more complete example, see
//! [Simple HDR Viewer application](https://github.com/iwikal/radiant/blob/master/examples/view_hdr.rs)
//!
//! Huge thanks to [HDRI Haven](https://hdrihaven.com) for providing CC0 sample images for testing!

// Original source: http://flipcode.com/archives/HDR_Image_Reader.shtml
use std::io::{BufRead, Error as IoError, ErrorKind};

mod dim_parser;
mod loader;

pub use loader::*;

/// The decoded R, G, and B value of a pixel. You typically get these from the data field on an
/// [`Image`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Rgb {
    /// The red channel.
    pub r: f32,
    /// The green channel.
    pub g: f32,
    /// The blue channel.
    pub b: f32,
}

unsafe impl bytemuck::Zeroable for Rgb {}
unsafe impl bytemuck::Pod for Rgb {}

impl Rgb {
    /// Construct an Rgb pixel with all channels set to zero, i.e. a black pixel.
    pub const fn zero() -> Self {
        Self {
            r: 0.,
            g: 0.,
            b: 0.,
        }
    }

    #[inline]
    fn apply_exposure(&mut self, expo: u8) {
        let expo = i32::from(expo) - 128;
        let d = 2_f32.powi(expo) / 255_f32;

        self.r *= d;
        self.g *= d;
        self.b *= d;
    }
}

#[derive(Debug, Clone)]
struct Rgbe {
    r: u8,
    g: u8,
    b: u8,
    e: u8,
}

impl std::convert::From<Rgbe> for Rgb {
    #[inline]
    fn from(rgbe: Rgbe) -> Self {
        let mut rgb = Self {
            r: rgbe.r as f32,
            g: rgbe.g as f32,
            b: rgbe.b as f32,
        };
        rgb.apply_exposure(rgbe.e);
        rgb
    }
}

impl std::convert::From<[u8; 4]> for Rgbe {
    #[inline]
    fn from([r, g, b, e]: [u8; 4]) -> Self {
        Self { r, g, b, e }
    }
}

impl std::convert::From<Rgbe> for [u8; 4] {
    #[inline]
    fn from(Rgbe { r, g, b, e }: Rgbe) -> Self {
        [r, g, b, e]
    }
}

impl Rgbe {
    #[inline]
    fn is_rle_marker(&self) -> bool {
        self.r == 1 && self.g == 1 && self.b == 1
    }

    #[inline]
    fn is_new_decrunch_marker(&self) -> bool {
        self.r == 2 && self.g == 2 && self.b & 128 == 0
    }
}

/// The various types of errors that can occur while loading an [`Image`].
#[derive(Debug)]
enum LoadError {
    Io(IoError),
    Eof,
    FileFormat,
    Header,
    Rle,
}

impl From<IoError> for LoadError {
    fn from(error: IoError) -> Self {
        match error.kind() {
            ErrorKind::UnexpectedEof => Self::Eof,
            _ => Self::Io(error),
        }
    }
}

impl From<LoadError> for IoError {
    fn from(error: LoadError) -> Self {
        let msg = match error {
            LoadError::Io(source) => return source,
            LoadError::Eof => return ErrorKind::UnexpectedEof.into(),
            LoadError::FileFormat => "the file is not a Radiance HDR image",
            LoadError::Header => "the image header is invalid",
            LoadError::Rle => "the image contained invalid run-length encoding",
        };

        Self::new(ErrorKind::InvalidData, msg)
    }
}

/// An alias for the type of results this crate returns.
type LoadResult<T = ()> = Result<T, LoadError>;

trait ReadExt {
    fn read_byte(&mut self) -> std::io::Result<u8>;
    fn read_rgbe(&mut self) -> std::io::Result<Rgbe>;
}

impl<R: BufRead> ReadExt for R {
    #[inline]
    fn read_byte(&mut self) -> std::io::Result<u8> {
        let mut buf = [0u8];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    #[inline]
    fn read_rgbe(&mut self) -> std::io::Result<Rgbe> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(buf.into())
    }
}

/// A decoded Radiance HDR image.
#[derive(Debug)]
pub struct Image {
    /// The width of the image, in pixels.
    pub width: usize,
    /// The height of the image, in pixels.
    pub height: usize,
    /// The decoded image data.
    pub data: Vec<Rgb>,
}

impl Image {
    /// Calculate an offset into the data buffer, given an x and y coordinate.
    pub fn pixel_offset(&self, x: usize, y: usize) -> usize {
        self.width * y + x
    }

    /// Get a pixel at a specific x and y coordinate. Will panic if out of bounds.
    pub fn pixel(&self, x: usize, y: usize) -> &Rgb {
        let offset = self.pixel_offset(x, y);
        &self.data[offset]
    }
}

/// Load a Radiance HDR image from a reader that implements [`BufRead`].
pub fn load<R: BufRead>(reader: R) -> Result<Image, IoError> {
    Loader::new(reader)?.load_image()
}
