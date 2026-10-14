//! Lectura/escritura little-endian con chequeo de límites.

use crate::{IoError, Result};

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| IoError::Corrupt("unexpected end of data".into()))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub fn rest(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos.min(self.data.len())..];
        self.pos = self.data.len();
        slice
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn string(&mut self) -> Result<String> {
        let len = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
    }
}

/// Escritura little-endian sobre un `Vec<u8>`.
pub(crate) trait Writer {
    fn w_u8(&mut self, v: u8);
    fn w_u16(&mut self, v: u16);
    fn w_i16(&mut self, v: i16);
    fn w_u32(&mut self, v: u32);
    fn w_zeros(&mut self, n: usize);
    fn w_string(&mut self, s: &str);
}

impl Writer for Vec<u8> {
    fn w_u8(&mut self, v: u8) {
        self.push(v);
    }

    fn w_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }

    fn w_i16(&mut self, v: i16) {
        self.extend_from_slice(&v.to_le_bytes());
    }

    fn w_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }

    fn w_zeros(&mut self, n: usize) {
        self.resize(self.len() + n, 0);
    }

    fn w_string(&mut self, s: &str) {
        let bytes = &s.as_bytes()[..s.len().min(u16::MAX as usize)];
        self.w_u16(bytes.len() as u16);
        self.extend_from_slice(bytes);
    }
}
