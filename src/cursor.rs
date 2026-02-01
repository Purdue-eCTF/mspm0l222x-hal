use core::fmt::{self, Write};

/// The cursor that is used to read and write to and from buffers of type T, coordinated by an offset.
pub struct Cursor<T> {
    buf: T,
    /// The value that the cursor location is offset by.
    pub offset: usize,
}

/// The error that is returned if the cursor attempts an invalid operation.
#[derive(Debug)]
pub enum CursorError {
    /// There is not enough bytes in the cursor or buffer that is being read from
    /// compared to the size of the buffer or cursor that is being read to.
    OversizeError(usize),
}

impl<T> Cursor<T> {
    /// Creates new Cursor with type T and offset 0.
    pub fn new(buf: T) -> Self {
        Cursor { buf, offset: 0 }
    }
}

impl<T> Cursor<T>
where
    T: AsRef<[u8]>,
{
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes in the cursor to do so, 
    /// return an error with how many bytes are left in the cursor.
    pub fn read_into(&mut self, other: &mut [u8]) -> Result<(), CursorError> {
        let remainder = &self.buf.as_ref()[self.offset..]; // get bytes from cursor [0,1/2,3]
        if remainder.len() < other.len() { // if the amount of bytes from cursor is less than length of buffer
            Err(CursorError::OversizeError(remainder.len())) // throw error with the amt of bytes
        } else {
            other.copy_from_slice(&remainder[..other.len()]); // copy from cursor to buffer
            self.offset += other.len(); // set offset to length of amount written
            Ok(())
        }
    }
}

impl<T> Cursor<T>
where
    T: AsMut<[u8]>,
{
    /// Read bytes from a buffer into this cursor.
    /// If there is not enough bytes in the buffer to do so, 
    /// return an error with how many bytes are left in the cursor.
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

    /// Retrieves the values that would be written into the cursor.
    pub fn written(&mut self) -> &mut [u8] {
        &mut self.buf.as_mut()[..self.offset]
    }
}

impl<T> Write for Cursor<T>
where
    T: AsMut<[u8]>,
{
    /// Read bytes from a buffer into this cursor.
    /// If there is not enough bytes in the buffer to do so, return a simple error.
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
