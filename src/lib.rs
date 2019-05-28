// Original source: http://flipcode.com/archives/HDR_Image_Reader.shtml
use std::io::{Read, Error as IoError};

#[derive(Debug, Clone)]
pub struct RGB {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone)]
struct RGBE {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub e: u8,
}

impl std::convert::From<[u8;4]> for RGBE {
    fn from(src: [u8;4]) -> Self {
        Self {
            r: src[0],
            g: src[1],
            b: src[2],
            e: src[3],
        }
    }
}

impl RGBE {
    pub fn is_rle_marker(&self) -> bool {
        self.r == 1 && self.g == 1 && self.b == 1
    }
}

fn process_rgbe(data: &[RGBE]) -> Vec<RGB> {
    data.iter().map(|rgbe| {
        let expo = i32::from(rgbe.e) - 128;
        let d = 2f32.powi(expo);

        let convert_component = |x: u8| {
            let v = f32::from(x) / 255f32;
            d * v
        };

        let r = convert_component(rgbe.r);
        let g = convert_component(rgbe.g);
        let b = convert_component(rgbe.b);
        RGB{r, g, b}
    }).collect()
}

#[derive(Debug)]
pub enum LoadError {
    Io(IoError),
    FileFormat,
    Rle,
}

impl std::convert::From<IoError> for LoadError {
    fn from(e: IoError) -> Self {
        LoadError::Io(e)
    }
}

pub type LoadResult<T> = Result<T, LoadError>;

trait ReadByte {
    fn read_byte(&mut self) -> std::io::Result<u8>;
}

impl<R: Read> ReadByte for R {
    fn read_byte(&mut self) -> std::io::Result<u8> {
        let mut buf = [0u8];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

fn old_decrunch<R: Read>(mut r: R, len: usize) -> LoadResult<Vec<RGBE>> {
    let mut result = Vec::<RGBE>::with_capacity(len);
    let mut r_shift = 0;
    let mut left = len;
    while left > 0 {
        let mut rgbe = [0u8; 4];
        r.read_exact(&mut rgbe[..])?;
        let rgbe = RGBE::from(rgbe);
        if rgbe.is_rle_marker() {
            let count = i32::from(rgbe.e) << r_shift;
            let val = result.last().ok_or(LoadError::Rle)?.clone();

            for _ in 0 .. count {
                result.push(val.clone());
            }
            left -= count as usize;
            r_shift += 8;
        } else {
            result.push(rgbe);
            left -= 1;
            r_shift = 0;
        }
    }
    Ok(result)
}

#[allow(clippy::many_single_char_names)]
fn decrunch<R: Read>(mut r: R, len: usize) -> LoadResult<Vec<RGBE>> {
    const MIN_LEN: usize = 8;
    const MAX_LEN: usize = 0x7fff;

    if len < MIN_LEN || len > MAX_LEN {
        return old_decrunch(r, len);
    }

    let i = r.read_byte()?;
    if i != 2 {
        let slice = &[i];
        let c = std::io::Cursor::new(slice);
        return old_decrunch(c.chain(r), len);
    }

    let g = r.read_byte()?;
    let b = r.read_byte()?;
    let i = r.read_byte()?;
    if g != 2 || b & 128 != 0 {
        let a = RGBE{ r: 2, g, b, e: i };
        let b = old_decrunch(r, len - 1)?;
        return Ok([a].iter().cloned().chain(b.into_iter()).collect());
    }

    let buf_len = len * 4;
    let mut buf = vec![0u8; buf_len];
    let buf_offset = |e, p| {
        p * 4 + e
    };
    
    for element_index in 0 .. 4 {
        let mut pixel_index = 0;
        while pixel_index < len {
            let code = r.read_byte()?;
            if code > 128 { // run
                let run_len = code & 127;
                let val = r.read_byte()?;
                for _ in 0 .. run_len {
                    let offset = buf_offset(element_index, pixel_index);
                    if offset >= buf.len() {
                        return Err(LoadError::Rle);
                    }

                    buf[offset] = val;
                    pixel_index += 1;
                }
            } else { // non-run
                let mut tmp = vec![0u8; code as usize];
                r.read_exact(tmp.as_mut_slice())?;
                for x in tmp {
                    let offset = buf_offset(element_index, pixel_index);
                    if offset >= buf.len() {
                        return Err(LoadError::Rle);
                    }
                                        
                    buf[offset] = x;
                    pixel_index += 1;
                }
            }
        }
    }

    let result = buf.chunks_exact(4).map(|x| {
        RGBE{r: x[0], g: x[1], b: x[2], e: x[3]}
    }).collect();
    Ok(result)
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

pub fn load<R: Read>(mut r: R) -> LoadResult<Image> {
    const MAGIC: &[u8; 10] = b"#?RADIANCE";
    const EOL: u8 = 0x0A;
    let mut buf = [0u8; 11];    // use MAGIC.len() + 1 when const slice len gets stabilized
    r.read_exact(&mut buf[..])?;
    
    if &buf[..MAGIC.len()] != MAGIC {
        return Err(LoadError::FileFormat);
    }
    
    // Skip the header
    {
        let mut old_c = 0u8;

        loop {
            let c = r.read_byte()?;
            if c == EOL && old_c == EOL {
                break;
            }
            old_c = c;
        }
    }

    // Grab image dimensions
    let (w, h) = {
        let mut buf = Vec::with_capacity(200);

        loop {
            let c = r.read_byte()?;
            if c == EOL {
                break;
            }
            buf.push(c);
        }

        let buf = String::from_utf8_lossy(buf.as_slice());
        let mut i = buf.split_ascii_whitespace();

        let marker = i.next().ok_or(LoadError::FileFormat)?;
        if marker != "-Y" {
            return Err(LoadError::FileFormat);
        }
        let h = i.next()
            .and_then(|x| x.parse::<usize>().ok())
            .ok_or(LoadError::FileFormat)?;

        let marker = i.next().ok_or(LoadError::FileFormat)?;
        if marker != "+X" {
            return Err(LoadError::FileFormat);
        }
        let w = i.next()
            .and_then(|x| x.parse::<usize>().ok())
            .ok_or(LoadError::FileFormat)?;

        (w, h)
    };

    let len = w * h;
    let mut data = Vec::with_capacity(len as usize);

    for _ in 0 .. h {   // Should be flipped
        let row_data = decrunch(r.by_ref(), w as usize)?;
        let row_data = process_rgbe(row_data.as_slice());
        data.extend(row_data);
    }

    let result = Image {
        width: w,
        height: h,
        data,
    };

    Ok(result)
}

