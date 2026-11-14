//! Byte-level encoding shared by every packet and message: little-endian
//! fixed-width integers, LEB128 varints, zigzag signed varints and
//! length-prefixed byte strings.
//!
//! Rules:
//! - Every read is range-checked. A short buffer, an overlong varint, a length
//!   past the end or over the caller's limit, or bad UTF-8 is a [`WireError`];
//!   nothing panics on hostile input.
//! - Readers never allocate more than the bytes actually present.

use std::fmt;

/// Why a datagram or message could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The buffer ended before the field.
    Short,
    /// A varint longer than its type allows.
    Overlong,
    /// A length or count over its limit.
    TooLong,
    /// An enum tag or field value out of range.
    BadValue,
    /// A string that is not UTF-8.
    BadText,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WireError::Short => "truncated",
            WireError::Overlong => "overlong varint",
            WireError::TooLong => "length over limit",
            WireError::BadValue => "value out of range",
            WireError::BadText => "text is not UTF-8",
        };
        f.write_str(s)
    }
}

impl std::error::Error for WireError {}

pub type WireResult<T> = Result<T, WireError>;

/// Appends encoded fields to a byte vector.
#[derive(Default, Debug)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            buf: Vec::with_capacity(n),
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn var(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
    }

    pub fn ivar(&mut self, v: i64) {
        self.var(((v << 1) ^ (v >> 63)) as u64);
    }

    pub fn bytes(&mut self, b: &[u8]) {
        self.var(b.len() as u64);
        self.buf.extend_from_slice(b);
    }

    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    pub fn raw(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
}

/// Reads fields from a byte slice, failing on anything out of range.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn is_done(&self) -> bool {
        self.pos == self.buf.len()
    }

    /// Fails unless every byte was consumed.
    pub fn finish(&self) -> WireResult<()> {
        if self.is_done() {
            Ok(())
        } else {
            Err(WireError::TooLong)
        }
    }

    pub fn raw(&mut self, n: usize) -> WireResult<&'a [u8]> {
        if self.remaining() < n {
            return Err(WireError::Short);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> WireResult<u8> {
        Ok(self.raw(1)?[0])
    }

    pub fn u16(&mut self) -> WireResult<u16> {
        let b = self.raw(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> WireResult<u32> {
        let b = self.raw(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> WireResult<u64> {
        let b = self.raw(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    pub fn var(&mut self) -> WireResult<u64> {
        let mut v: u64 = 0;
        for i in 0..10 {
            let b = self.u8()?;
            let part = u64::from(b & 0x7f);
            if i == 9 && part > 1 {
                return Err(WireError::Overlong);
            }
            v |= part << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(WireError::Overlong)
    }

    pub fn var32(&mut self) -> WireResult<u32> {
        u32::try_from(self.var()?).map_err(|_| WireError::Overlong)
    }

    pub fn ivar(&mut self) -> WireResult<i64> {
        let v = self.var()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    pub fn i32(&mut self) -> WireResult<i32> {
        i32::try_from(self.ivar()?).map_err(|_| WireError::BadValue)
    }

    pub fn i16(&mut self) -> WireResult<i16> {
        i16::try_from(self.ivar()?).map_err(|_| WireError::BadValue)
    }

    /// A length-prefixed byte string of at most `max` bytes.
    pub fn bytes(&mut self, max: usize) -> WireResult<&'a [u8]> {
        let n = self.var()?;
        if n > max as u64 {
            return Err(WireError::TooLong);
        }
        self.raw(n as usize)
    }

    /// A length-prefixed UTF-8 string of at most `max` bytes.
    pub fn str(&mut self, max: usize) -> WireResult<String> {
        let b = self.bytes(max)?;
        std::str::from_utf8(b)
            .map(str::to_owned)
            .map_err(|_| WireError::BadText)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_varints_round_trip() {
        let vals = [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX];
        let ivals = [0i64, -1, 1, -64, 64, i64::MIN, i64::MAX];
        let mut w = Writer::new();
        for &v in &vals {
            w.var(v);
        }
        for &v in &ivals {
            w.ivar(v);
        }
        let mut r = Reader::new(&w.buf);
        for &v in &vals {
            assert_eq!(r.var().unwrap(), v);
        }
        for &v in &ivals {
            assert_eq!(r.ivar().unwrap(), v);
        }
        r.finish().unwrap();
    }

    #[test]
    fn wire_rejects_overlong_and_short() {
        let over = [0xffu8; 11];
        assert_eq!(Reader::new(&over).var(), Err(WireError::Overlong));
        let mut w = Writer::new();
        w.bytes(&[1, 2, 3]);
        assert_eq!(Reader::new(&w.buf).bytes(2), Err(WireError::TooLong));
        assert_eq!(Reader::new(&w.buf[..2]).bytes(8), Err(WireError::Short));
        assert_eq!(Reader::new(&[0xff, 0xfe]).u32(), Err(WireError::Short));
    }
}
