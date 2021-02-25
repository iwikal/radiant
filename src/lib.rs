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
use std::io::{BufRead, Error as IoError, ErrorKind, Read};

/// The decoded R, G, and B value of a pixel. You typically get these from the data field on an
/// [`Image`].
#[derive(Debug, Clone)]
pub struct RGB {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RGB {
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
struct RGBE {
    r: u8,
    g: u8,
    b: u8,
    e: u8,
}

impl std::convert::From<RGBE> for RGB {
    #[inline]
    fn from(rgbe: RGBE) -> Self {
        let mut rgb = Self {
            r: rgbe.r as f32,
            g: rgbe.g as f32,
            b: rgbe.b as f32,
        };
        rgb.apply_exposure(rgbe.e);
        rgb
    }
}

impl std::convert::From<[u8; 4]> for RGBE {
    #[inline]
    fn from([r, g, b, e]: [u8; 4]) -> Self {
        Self { r, g, b, e }
    }
}

impl RGBE {
    #[inline]
    fn is_rle_marker(&self) -> bool {
        self.r == 1 && self.g == 1 && self.b == 1
    }
}

/// The various types of errors that can occur while loading an [`Image`].
#[derive(thiserror::Error, Debug)]
pub enum LoadError {
    /// A lower level io error was raised.
    #[error("io error: {0}")]
    Io(#[source] IoError),
    /// The image file ended unexpectedly.
    #[error("file ended unexpectedly")]
    Eof(#[source] IoError),
    /// The file did not follow valid Radiance HDR format.
    #[error("invalid file format")]
    FileFormat,
    /// The image file contained invalid run-length encoding.
    #[error("invalid run-length encoding")]
    Rle,
}

impl From<IoError> for LoadError {
    fn from(error: IoError) -> Self {
        match error.kind() {
            ErrorKind::UnexpectedEof => Self::Eof(error),
            _ => Self::Io(error),
        }
    }
}

/// An alias for the type of results this crate returns.
pub type LoadResult<T = ()> = Result<T, LoadError>;

trait ReadByte {
    fn read_byte(&mut self) -> std::io::Result<u8>;
}

impl<R: BufRead> ReadByte for R {
    #[inline]
    fn read_byte(&mut self) -> std::io::Result<u8> {
        let mut buf = [0u8];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

fn old_decrunch<R: BufRead>(mut reader: R, mut scanline: &mut [RGB]) -> LoadResult {
    let mut r_shift = 0;

    while scanline.len() > 1 {
        let mut rgbe = [0u8; 4];
        reader.read_exact(&mut rgbe)?;
        let rgbe = RGBE::from(rgbe);
        if rgbe.is_rle_marker() {
            let count = usize::from(rgbe.e) << r_shift;
            if count >= scanline.len() {
                return Err(LoadError::Rle);
            }

            let from = scanline[0].clone();
            for to in &mut scanline[1..=count] {
                *to = from.clone();
            }

            scanline = &mut scanline[count..];
            r_shift += 8;
        } else {
            scanline[1] = rgbe.into();
            scanline = &mut scanline[1..];
            r_shift = 0;
        }
    }

    Ok(())
}

fn decrunch<R: BufRead>(mut reader: R, scanline: &mut [RGB]) -> LoadResult {
    const MIN_LEN: usize = 8;
    const MAX_LEN: usize = 0x7fff;

    if scanline.len() < MIN_LEN || scanline.len() > MAX_LEN {
        return old_decrunch(reader, scanline);
    }

    let r = reader.read_byte()?;
    if r != 2 {
        let slice = &[r];
        let c = std::io::Cursor::new(slice);
        return old_decrunch(c.chain(reader), scanline);
    }

    let [g, b, e] = {
        let mut buf = [0; 3];
        reader.read_exact(&mut buf)?;
        buf
    };

    if g != 2 || b & 128 != 0 {
        let first = &mut scanline[0];
        first.r = 2.0;
        first.g = g as f32;
        first.b = b as f32;
        first.apply_exposure(e);

        return old_decrunch(reader, scanline);
    }

    for element_index in 0..4 {
        let mut pixel_index = 0;
        while pixel_index < scanline.len() {
            let mut write = |val: u8| -> LoadResult {
                let pixel = scanline.get_mut(pixel_index).ok_or(LoadError::Rle)?;
                match element_index {
                    0 => pixel.r = val as f32,
                    1 => pixel.g = val as f32,
                    2 => pixel.b = val as f32,
                    _ => pixel.apply_exposure(val),
                }
                pixel_index += 1;
                Ok(())
            };

            let code = reader.read_byte()?;
            if code > 128 {
                // run
                let run_len = code & 127;
                let val = reader.read_byte()?;
                for _ in 0..run_len {
                    write(val)?;
                }
            } else {
                // non-run
                let mut bytes_left = code as usize;
                while bytes_left > 0 {
                    let buf = reader.fill_buf()?;
                    if buf.is_empty() {
                        return Err(LoadError::Rle);
                    }
                    let count = buf.len().min(bytes_left);
                    for &val in &buf[..count] {
                        write(val)?;
                    }
                    reader.consume(count);
                    bytes_left -= count;
                }
            }
        }
    }

    Ok(())
}

/// A decoded Radiance HDR image.
#[derive(Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<RGB>,
}

impl Image {
    /// Calculate an offset into the data buffer, given an x and y coordinate.
    pub fn pixel_offset(&self, x: usize, y: usize) -> usize {
        self.width * y + x
    }

    /// Get a pixel at a specific x and y coordinate. Will panic if out of bounds.
    pub fn pixel(&self, x: usize, y: usize) -> &RGB {
        let offset = self.pixel_offset(x, y);
        &self.data[offset]
    }
}

const MAGIC: &[u8; 10] = b"#?RADIANCE";
const EOL: u8 = 0xA;

/// Load a Radiance HDR image from a reader that implements [`BufRead`].
pub fn load<R: BufRead>(mut reader: R) -> LoadResult<Image> {
    let mut buf = [0u8; MAGIC.len() + 1];
    reader.read_exact(&mut buf[..])?;

    if &buf[..MAGIC.len()] != MAGIC {
        return Err(LoadError::FileFormat);
    }

    // Skip header
    loop {
        let mut eol = || reader.read_byte().map(|byte| byte == EOL);

        if eol()? && eol()? {
            break;
        }
    }

    // Grab image dimensions
    let (width, height, mut reader) = DimParser::parse(reader)?;

    // Allocate result buffer
    let mut data = vec![
        RGB {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
        width * height
    ];

    // Decrunch image data
    for row in 0..height {
        let start = row * width;
        let end = start + width;
        decrunch(&mut reader, &mut data[start..end])?;
    }

    Ok(Image {
        width,
        height,
        data,
    })
}

struct DimParser<R> {
    reader: R,
    byte: u8,
}

impl<R: BufRead> DimParser<R> {
    fn new(mut reader: R) -> LoadResult<Self> {
        let byte = reader.read_byte()?;
        Ok(Self { reader, byte })
    }

    fn eat(&mut self) -> LoadResult<u8> {
        self.byte = self.reader.read_byte()?;
        Ok(self.byte)
    }

    fn eat_whitespace(&mut self) -> LoadResult {
        loop {
            if self.byte == EOL {
                return Err(LoadError::FileFormat);
            } else if self.byte.is_ascii_whitespace() {
                self.byte = self.reader.read_byte()?;
                continue;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn expect_whitespace(&mut self) -> LoadResult {
        if self.byte.is_ascii_whitespace() {
            self.eat_whitespace()
        } else {
            Err(LoadError::FileFormat)
        }
    }

    fn expect_usize(&mut self) -> LoadResult<usize> {
        let mut value = 0;
        if !self.byte.is_ascii_digit() {
            return Err(LoadError::FileFormat);
        }
        loop {
            value *= 10;
            value += (self.byte - b'0') as usize;
            if !self.eat()?.is_ascii_digit() {
                return Ok(value);
            }
        }
    }

    fn expect(&mut self, byte: u8) -> LoadResult {
        match self.byte == byte {
            true => {
                self.eat()?;
                Ok(())
            }
            false => Err(LoadError::FileFormat),
        }
    }

    fn expect_y(&mut self) -> LoadResult<usize> {
        self.expect(b'-')?;
        self.expect(b'Y')?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_x(&mut self) -> LoadResult<usize> {
        self.expect(b'+')?;
        self.expect(b'X')?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_eol(&mut self) -> LoadResult {
        match self.byte {
            EOL => Ok(()),
            _ => Err(LoadError::FileFormat),
        }
    }

    fn parse_impl(mut self) -> LoadResult<(usize, usize, R)> {
        self.eat_whitespace()?;
        let y = self.expect_y()?;
        self.expect_whitespace()?;
        let x = self.expect_x()?;

        while self.byte != EOL {
            if !self.byte.is_ascii_whitespace() {
                return Err(LoadError::FileFormat);
            }
            self.eat()?;
        }

        self.expect_eol()?;
        Ok((x, y, self.reader))
    }

    fn parse(reader: R) -> LoadResult<(usize, usize, R)> {
        Self::new(reader)?.parse_impl()
    }
}
