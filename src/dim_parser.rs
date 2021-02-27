use super::{LoadError, LoadResult, ReadByte};
use std::io::BufRead;

const EOL: u8 = 0xA;

pub(crate) fn parse_header<R: BufRead>(mut reader: R) -> LoadResult<(usize, usize, R)> {
    // Skip first paragraph
    loop {
        let mut next_is_eol = || reader.read_byte().map(|b| b == EOL);
        if next_is_eol()? && next_is_eol()? {
            break;
        }
    }

    DimParser::new(reader)?.parse()
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

    fn parse(mut self) -> LoadResult<(usize, usize, R)> {
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

    fn eat(&mut self) -> LoadResult<u8> {
        self.byte = self.reader.read_byte()?;
        Ok(self.byte)
    }

    fn expect<B: AsRef<[u8]>>(&mut self, bytes: B) -> LoadResult {
        for &byte in bytes.as_ref() {
            if self.byte == byte {
                self.eat()?;
            } else {
                return Err(LoadError::FileFormat);
            }
        }
        Ok(())
    }

    fn expect_y(&mut self) -> LoadResult<usize> {
        self.expect(b"-Y")?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_x(&mut self) -> LoadResult<usize> {
        self.expect(b"+X")?;
        self.expect_whitespace()?;
        self.expect_usize()
    }

    fn expect_usize(&mut self) -> LoadResult<usize> {
        let mut value: usize = 0;
        if !self.byte.is_ascii_digit() {
            return Err(LoadError::FileFormat);
        }
        loop {
            value = value.checked_mul(10).ok_or(LoadError::FileFormat)?;
            value += (self.byte - b'0') as usize;
            if !self.eat()?.is_ascii_digit() {
                return Ok(value);
            }
        }
    }

    fn expect_eol(&mut self) -> LoadResult {
        match self.byte {
            EOL => Ok(()),
            _ => Err(LoadError::FileFormat),
        }
    }
}
