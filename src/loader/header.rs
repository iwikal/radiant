use super::{LoadError, LoadResult, ReadExt};
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
        self.eat_spaces()?;
        let y = self.expect_y()?;
        self.expect_spaces()?;
        let x = self.expect_x()?;
        self.eat_spaces()?;
        self.expect_eol()?;
        Ok((x, y, self.reader))
    }

    fn eat_spaces(&mut self) -> LoadResult<bool> {
        let mut ate_any = false;
        while self.byte == b' ' {
            ate_any = true;
            self.eat()?;
        }
        Ok(ate_any)
    }

    fn expect_spaces(&mut self) -> LoadResult {
        match self.eat_spaces()? {
            true => Ok(()),
            false => Err(LoadError::Header),
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
                return Err(LoadError::Header);
            }
        }
        Ok(())
    }

    fn expect_y(&mut self) -> LoadResult<usize> {
        self.expect(b"-Y")?;
        self.expect_spaces()?;
        self.expect_usize()
    }

    fn expect_x(&mut self) -> LoadResult<usize> {
        self.expect(b"+X")?;
        self.expect_spaces()?;
        self.expect_usize()
    }

    fn expect_usize(&mut self) -> LoadResult<usize> {
        let mut value: usize = 0;
        if !self.byte.is_ascii_digit() {
            return Err(LoadError::Header);
        }
        loop {
            value = value
                .checked_mul(10)
                .ok_or(LoadError::Header)?
                .checked_add((self.byte - b'0') as usize)
                .ok_or(LoadError::Header)?;
            if !self.eat()?.is_ascii_digit() {
                return Ok(value);
            }
        }
    }

    fn expect_eol(&mut self) -> LoadResult {
        match self.byte {
            EOL => Ok(()),
            _ => Err(LoadError::Header),
        }
    }
}
