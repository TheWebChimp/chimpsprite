//! Un `IStream` de COM como `Read + Seek`, para que el zip lea solo lo que necesita.

use std::io::{self, Read, Seek, SeekFrom};

use windows::Win32::System::Com::{IStream, STREAM_SEEK_CUR, STREAM_SEEK_END, STREAM_SEEK_SET};

pub struct StreamReader(pub IStream);

fn io_error(e: windows_core::Error) -> io::Error {
    io::Error::other(e.message())
}

impl Read for StreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut read = 0u32;
        // S_FALSE (menos bytes que los pedidos) es el fin del stream, no un error.
        unsafe { self.0.Read(buf.as_mut_ptr().cast(), want, Some(&mut read)) }
            .ok()
            .map_err(io_error)?;
        Ok(read as usize)
    }
}

impl Seek for StreamReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (offset, origin) = match pos {
            SeekFrom::Start(n) => (
                i64::try_from(n).map_err(|_| io::Error::other("seek out of range"))?,
                STREAM_SEEK_SET,
            ),
            SeekFrom::Current(n) => (n, STREAM_SEEK_CUR),
            SeekFrom::End(n) => (n, STREAM_SEEK_END),
        };
        let mut position = 0u64;
        unsafe { self.0.Seek(offset, origin, Some(&mut position)) }.map_err(io_error)?;
        Ok(position)
    }
}
