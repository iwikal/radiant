use crate::{Image, LoadError, LoadResult, ReadExt, Rgb};
use std::io::{BufRead, Error as IoError, ErrorKind};

mod header;

const MAGIC: &[u8; 10] = b"#?RADIANCE";

/// A struct that represents an image in the process of being loaded.
pub struct Loader<R> {
    /// The width of the image, in pixels.
    pub width: usize,
    /// The height of the image, in pixels.
    pub height: usize,
    reader: R,
}

impl<R: BufRead> Loader<R> {
    /// Construct a new [`Loader`]. This will consume the header from the provided reader.
    pub fn new(mut reader: R) -> Result<Self, IoError> {
        let mut buf = [0u8; MAGIC.len()];
        reader.read_exact(&mut buf).map_err(LoadError::from)?;

        if &buf != MAGIC {
            return Err(LoadError::FileFormat.into());
        }

        // Grab image dimensions
        let (width, height, reader) = header::parse_header(reader)?;

        Ok(Self {
            width,
            height,
            reader,
        })
    }

    /// Convert this loader into an [`ScanlinesLoader`], which lets you load the image one scanline at a time.
    pub fn scanlines(self) -> ScanlinesLoader<R> {
        ScanlinesLoader {
            width: self.width,
            height: self.height,
            reader: self.reader,
        }
    }

    /// Load an entire [`Image`] at once.
    pub fn load_image(self) -> Result<Image, IoError> {
        let &Self { width, height, .. } = &self;
        let length = width.checked_mul(height).ok_or(LoadError::Header)?;

        let mut data = vec![Rgb::zero(); length];

        if length != 0 {
            let mut scanlines = self.scanlines();

            for y in 0..height {
                let start = y * width;
                scanlines.read_scanline(&mut data[start..])?;
            }
        }

        Ok(Image {
            width,
            height,
            data,
        })
    }
}

/// An image loader that decodes images line by line, through an iterative API.
/// ```rust
/// use radiant::{Rgb, Loader};
/// use std::io::BufReader;
/// use std::fs::File;
///
/// let f = File::open("assets/colorful_studio_2k.hdr").expect("failed to open file");
/// let f = BufReader::new(f);
/// let mut loader = Loader::new(f)
///     .expect("failed to read image")
///     .scanlines();
/// let height = loader.height;
/// let width = loader.width;
///
/// // Allocate a buffer that fits one whole scanline (or more)
/// let mut buffer = vec![Rgb::zero(); width];
/// for y in 0..height {
///     loader.read_scanline(&mut buffer).expect("failed to read image");
///     // do something with the decoded scanline, such as uploading it to a GPU texture
/// }
///
/// // If you enable the "impl-bytemuck" feature,
/// // you can use the bytemuck crate to cast the buffer to another plain-old-data type.
/// // This layout is guaranteed, so you can also do this using `unsafe`.
/// # #[cfg(feature = "impl-bytemuck")]
/// let buffer: &[[f32; 3]] = bytemuck::cast_slice(&buffer);
/// ```
pub struct ScanlinesLoader<R> {
    /// The width of the image.
    pub width: usize,
    /// The height of the image, i.e. the number of scanlines.
    pub height: usize,
    reader: R,
}

impl<R: BufRead> ScanlinesLoader<R> {
    /// Decode image data into the next horizontal scanline of the image. The provided scanline
    /// buffer must be at least as long as the width of the image, otherwise an error of the kind
    /// [`std::io::ErrorKind::InvalidInput`] will be returned.
    pub fn read_scanline(&mut self, scanline: &mut [Rgb]) -> Result<(), IoError> {
        let scanline = scanline
            .get_mut(..self.width)
            .ok_or_else(Self::invalid_input)?;

        if !scanline.is_empty() {
            const MIN_LEN: usize = 8;
            const MAX_LEN: usize = 0x7fff;

            let rgbe = self.reader.read_rgbe()?;

            if (MIN_LEN..=MAX_LEN).contains(&scanline.len()) && rgbe.is_new_decrunch_marker() {
                self.new_decrunch(scanline)?;
            } else {
                scanline[0] = rgbe.into();
                self.old_decrunch(scanline)?;
            }
        }

        Ok(())
    }

    fn invalid_input() -> IoError {
        IoError::new(
            ErrorKind::InvalidInput,
            "image width exceeded length of provided buffer",
        )
    }

    fn old_decrunch(&mut self, mut scanline: &mut [Rgb]) -> LoadResult {
        let mut l_shift = 0;

        while scanline.len() > 1 {
            let rgbe = self.reader.read_rgbe()?;
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

    fn new_decrunch(&mut self, scanline: &mut [Rgb]) -> LoadResult {
        let mut decrunch_channel = |mutate_pixel: fn(&mut Rgb, u8)| -> LoadResult<()> {
            let mut scanline = &mut *scanline;
            while !scanline.is_empty() {
                let code = self.reader.read_byte()? as usize;
                if code > 128 {
                    // run
                    let val = self.reader.read_byte()?;

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
                        let buf = self.reader.fill_buf()?;

                        if buf.is_empty() {
                            return Err(LoadError::Eof);
                        }

                        let count = buf.len().min(bytes_left);
                        scanline
                            .get_mut(..count)
                            .ok_or(LoadError::Rle)?
                            .iter_mut()
                            .zip(buf)
                            .for_each(|(pixel, &val)| mutate_pixel(pixel, val));

                        scanline = &mut scanline[count..];
                        self.reader.consume(count);
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
}

struct ScanlinesIter<R> {
    loader: ScanlinesLoader<R>,
}

impl<R: BufRead> Iterator for ScanlinesIter<R> {
    type Item = Result<Vec<Rgb>, IoError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.loader.height {
            0 => None,
            _ => Some({
                self.loader.height -= 1;
                let mut buf = vec![Rgb::zero(); self.loader.width];
                match self.loader.read_scanline(&mut buf) {
                    Ok(_) => Ok(buf),
                    Err(e) => {
                        self.loader.height = 0;
                        Err(e)
                    }
                }
            }),
        }
    }
}
