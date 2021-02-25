// Original source: http://flipcode.com/archives/HDR_Image_Reader.shtml
use std::io::{Error as IoError, BufRead, Read};

#[derive(Debug, Clone, Copy)]
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
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub e: u8,
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
    pub fn is_rle_marker(&self) -> bool {
        self.r == 1 && self.g == 1 && self.b == 1
    }
}

#[derive(Debug)]
pub enum LoadError {
    Io(IoError),
    FileFormat,
    Rle,
}

impl From<IoError> for LoadError {
    fn from(e: IoError) -> Self {
        Self::Io(e)
    }
}

pub type LoadResult<T> = Result<T, LoadError>;

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

fn old_decrunch<R: BufRead>(mut reader: R, scanline: &mut [RGB]) -> LoadResult<()> {
    let mut index = 0;
    let mut r_shift = 0;

    while index < scanline.len() {
        let mut rgbe = [0u8; 4];
        reader.read_exact(&mut rgbe[..])?;
        let rgbe = RGBE::from(rgbe);
        if rgbe.is_rle_marker() {
            let count = usize::from(rgbe.e) << r_shift;
            let val = *scanline.get(index - 1).ok_or(LoadError::Rle)?;
            for _ in 0..count {
                scanline[index] = val;
            }
            index += count;
            r_shift += 8;
        } else {
            scanline[index] = rgbe.into();
            index += 1;
            r_shift = 0;
        }
    }

    Ok(())
}

fn decrunch<R: BufRead>(mut reader: R, scanline: &mut [RGB]) -> LoadResult<()> {
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

        return old_decrunch(reader, &mut scanline[1..]);
    }

    for element_index in 0..4 {
        let mut pixel_index = 0;
        while pixel_index < scanline.len() {
            let mut write = |val: u8| -> LoadResult<()> {
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

#[derive(Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<RGB>,
}

impl Image {
    pub fn pixel_offset(&self, x: usize, y: usize) -> usize {
        self.width * y + x
    }

    pub fn pixel(&self, x: usize, y: usize) -> &RGB {
        let offset = self.pixel_offset(x, y);
        &self.data[offset]
    }
}

const MAGIC: &[u8; 10] = b"#?RADIANCE";
const EOL: u8 = 0xA;

pub fn load<R: BufRead>(mut reader: R) -> Result<Image, LoadError> {
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
    fn new(mut reader: R) -> Result<Self, LoadError> {
        let byte = reader.read_byte()?;
        Ok(Self { reader, byte })
    }

    fn eat(&mut self) -> Result<u8, LoadError> {
        self.byte = self.reader.read_byte()?;
        Ok(self.byte)
    }

    fn eat_whitespace(&mut self) -> Result<(), LoadError> {
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

    fn expect_whitespace(&mut self) -> Result<(), LoadError> {
        if self.byte.is_ascii_whitespace() {
            self.eat_whitespace()
        } else {
            Err(LoadError::FileFormat)
        }
    }

    fn expect_usize(&mut self) -> Result<usize, LoadError> {
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

    fn expect(&mut self, byte: u8) -> Result<(), LoadError> {
        match self.byte == byte {
            true => {
                self.eat()?;
                Ok(())
            }
            false => {
                Err(LoadError::FileFormat)
            }
        }
    }

    fn expect_y(&mut self) -> Result<usize, LoadError> {
        self.expect(b'-')?;
        self.expect(b'Y')?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_x(&mut self) -> Result<usize, LoadError> {
        self.expect(b'+')?;
        self.expect(b'X')?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_eol(&mut self) -> Result<(), LoadError> {
        match self.byte {
            EOL => Ok(()),
            _ => Err(LoadError::FileFormat),
        }
    }

    fn parse_impl(mut self) -> Result<(usize, usize, R), LoadError> {
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

    fn parse(reader: R) -> Result<(usize, usize, R), LoadError> {
        Self::new(reader)?.parse_impl()
    }
}
