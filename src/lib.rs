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

/// The decoded R, G, and B value of a pixel. You typically get these from the data field on an
/// [`Image`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    /// The red channel.
    pub r: f32,
    /// The green channel.
    pub g: f32,
    /// The blue channel.
    pub b: f32,
}

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
    FileFormat,
    Header,
    Rle,
}

impl From<IoError> for LoadError {
    fn from(error: IoError) -> Self {
        match error.kind() {
            kind @ ErrorKind::UnexpectedEof => Self::Io(kind.into()),
            _ => Self::Io(error),
        }
    }
}

impl From<LoadError> for IoError {
    fn from(error: LoadError) -> Self {
        let msg = match error {
            LoadError::Io(source) => return source,
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

fn old_decrunch<R: BufRead>(mut reader: R, mut scanline: &mut [Rgb]) -> LoadResult {
    let mut l_shift = 0;

    while scanline.len() > 1 {
        let rgbe = reader.read_rgbe()?;
        if rgbe.is_rle_marker() {
            let count = usize::checked_shl(1, l_shift)
                .and_then(|shift_factor| usize::from(rgbe.e).checked_mul(shift_factor))
                .ok_or(LoadError::Rle)?;

            let from = scanline[0];

            scanline
                .get_mut(1..=count)
                .ok_or(LoadError::Rle)?
                .iter_mut()
                .for_each(|to| *to = from);

            scanline = &mut scanline[count..];
            l_shift += 8;
        } else {
            scanline[1] = rgbe.into();
            scanline = &mut scanline[1..];
            l_shift = 0;
        }
    }

    Ok(())
}

fn decrunch<R: BufRead>(mut reader: R, scanline: &mut [Rgb]) -> LoadResult {
    if scanline.is_empty() {
        return Ok(());
    }

    const MIN_LEN: usize = 8;
    const MAX_LEN: usize = 0x7fff;

    let rgbe = reader.read_rgbe()?;

    if (MIN_LEN..=MAX_LEN).contains(&scanline.len()) && rgbe.is_new_decrunch_marker() {
        new_decrunch(reader, scanline)
    } else {
        scanline[0] = rgbe.into();
        old_decrunch(reader, scanline)
    }
}

fn new_decrunch<R: BufRead>(mut reader: R, scanline: &mut [Rgb]) -> LoadResult {
    let mut decrunch_channel = |mutate_pixel: fn(&mut Rgb, u8)| -> LoadResult<()> {
        let mut scanline = &mut scanline[..];
        while !scanline.is_empty() {
            let code = reader.read_byte()? as usize;
            if code > 128 {
                // run
                let val = reader.read_byte()?;

                let count = code & 127;
                scanline
                    .get_mut(..count)
                    .ok_or(LoadError::Rle)?
                    .iter_mut()
                    .for_each(|pixel| mutate_pixel(pixel, val));

                scanline = &mut scanline[count..];
            } else {
                // non-run
                let mut bytes_left = code;
                while bytes_left > 0 {
                    let buf = reader.fill_buf()?;

                    if buf.is_empty() {
                        return Err(LoadError::Io(ErrorKind::UnexpectedEof.into()));
                    }

                    let count = buf.len().min(bytes_left);
                    scanline
                        .get_mut(..count)
                        .ok_or(LoadError::Rle)?
                        .iter_mut()
                        .zip(buf)
                        .for_each(|(pixel, &val)| mutate_pixel(pixel, val));

                    scanline = &mut scanline[count..];
                    reader.consume(count);
                    bytes_left -= count;
                }
            }
        }

        Ok(())
    };

    decrunch_channel(|pixel, val| pixel.r = val as f32)?;
    decrunch_channel(|pixel, val| pixel.g = val as f32)?;
    decrunch_channel(|pixel, val| pixel.b = val as f32)?;
    decrunch_channel(Rgb::apply_exposure)
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

const MAGIC: &[u8; 10] = b"#?RADIANCE";

/// An image loader that decodes images line by line, through an iterative API.
/// ```rust
/// use radiant::{Rgb, IterLoader};
/// use std::io::BufReader;
/// use std::fs::File;
///
/// let f = File::open("assets/colorful_studio_2k.hdr").expect("failed to open file");
/// let f = BufReader::new(f);
/// let mut loader = IterLoader::new(f).expect("failed to read image");
/// let mut buffer = vec![Rgb::zero(); loader.width];
/// for y in 0..loader.height {
///     loader.read_scanline(&mut buffer).expect("failed to read image");
///     // do something with the decoded scanline, such as uploading it to a GPU texture
/// }
/// ```
pub struct IterLoader<R> {
    /// The width of the image.
    pub width: usize,
    /// The height of the image, i.e. the number of scanlines.
    pub height: usize,
    reader: R,
}

impl<R: BufRead> IterLoader<R> {
    /// Construct a new [`IterLoader`]. This will consume the header from the provided reader.
    pub fn new(mut reader: R) -> Result<Self, IoError> {
        let mut buf = [0u8; MAGIC.len()];
        reader.read_exact(&mut buf).map_err(LoadError::from)?;

        if &buf != MAGIC {
            Err(LoadError::FileFormat)?;
        }

        // Grab image dimensions
        let (width, height, reader) = dim_parser::parse_header(reader)?;

        Ok(Self {
            width,
            height,
            reader,
        })
    }

    /// Decode image data into the next horizontal scanline of the image. The provided scanline
    /// buffer must be at least as long as the width of the image, otherwise an error of the kind
    /// [`std::io::ErrorKind::InvalidInput`] will be returned.
    pub fn read_scanline(&mut self, scanline: &mut [Rgb]) -> Result<(), IoError> {
        let scanline = scanline
            .get_mut(..self.width)
            .ok_or_else(Self::invalid_input)?;
        decrunch(&mut self.reader, scanline)?;
        Ok(())
    }

    fn invalid_input() -> IoError {
        IoError::new(
            ErrorKind::InvalidInput,
            "image width exceeded length of provided buffer",
        )
    }

    /// The inner reader is not guaranteed to be empty when the image is completely decoded.
    /// This function can be used to retrieve the inner reader.
    pub fn into_inner_reader(self) -> R {
        self.reader
    }
}

/// Load a Radiance HDR image from a reader that implements [`BufRead`].
pub fn load<R: BufRead>(reader: R) -> Result<Image, IoError> {
    let mut loader = IterLoader::new(reader)?;
    let width = loader.width;
    let height = loader.height;

    let length = width.checked_mul(height).ok_or(LoadError::Header)?;

    let mut data = vec![Rgb::zero(); length];

    for row in 0..height {
        let start = row * width;
        loader.read_scanline(&mut data[start..])?;
    }

    Ok(Image {
        width,
        height,
        data,
    })
}
