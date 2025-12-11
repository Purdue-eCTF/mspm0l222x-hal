use core::fmt::{self, Write};

pub struct Cursor<T> {
    buf: T,
    pub offset: usize,
}

#[derive(Debug)]
pub enum CursorError {
    OversizeError(usize),
}

impl<T> Cursor<T> {
    pub fn new(buf: T) -> Self {
        Cursor { buf, offset: 0 }
    }
}

impl<T> Cursor<T>
where
    T: AsRef<[u8]>,
{
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes remaining to do so, return an error with how many bytes are left
    pub fn read_into(&mut self, other: &mut [u8]) -> Result<(), CursorError> {
        let remainder = &self.buf.as_ref()[self.offset..];
        if remainder.len() < other.len() {
            Err(CursorError::OversizeError(remainder.len()))
        } else {
            other.copy_from_slice(&remainder[..other.len()]);
            self.offset += other.len();
            Ok(())
        }
    }
}

impl<T> Cursor<T>
where
    T: AsMut<[u8]>,
{
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes remaining to do so, return an error with how many bytes are left
    pub fn read_from(&mut self, other: &[u8]) -> Result<(), CursorError> {
        let remainder = &mut self.buf.as_mut()[self.offset..];
        if remainder.len() < other.len() {
            Err(CursorError::OversizeError(remainder.len()))
        } else {
            let () = &mut remainder[..other.len()].copy_from_slice(other);
            self.offset += other.len();
            Ok(())
        }
    }

    pub fn written(&mut self) -> &mut [u8] {
        &mut self.buf.as_mut()[..self.offset]
    }
}

impl<T> Write for Cursor<T>
where
    T: AsMut<[u8]>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remainder = &mut self.buf.as_mut()[self.offset..];
        if remainder.len() < bytes.len() {
            return Err(fmt::Error);
        }
        let remainder = &mut remainder[..bytes.len()];
        remainder.copy_from_slice(bytes);
        self.offset += bytes.len();

        Ok(())
    }
}

