//! The little-endian `u32` count strings and vectors start with.

use crate::Error;

/// The count a string or vector starts with.
#[inline(always)]
pub(crate) fn count(bytes: &[u8]) -> Result<usize, Error> {
    let count = bytes.first_chunk::<4>().ok_or(Error::TooShort)?;
    Ok(u32::from_le_bytes(*count) as usize)
}

/// # Safety
///
/// `bytes` start with a count.
#[inline(always)]
pub(crate) unsafe fn count_unchecked(bytes: &[u8]) -> usize {
    // SAFETY: forwarded; an array of bytes has alignment 1.
    u32::from_le_bytes(unsafe { *bytes.as_ptr().cast() }) as usize
}

/// Fails if `len` passes the first of `limits`, or a `u32` count.
#[inline(always)]
pub(crate) fn within(len: usize, limits: &[usize]) -> Result<(), Error> {
    let limit = limits.first().copied().unwrap_or(u32::MAX as usize);
    if len <= limit && len <= u32::MAX as usize {
        Ok(())
    } else {
        Err(Error::TooLong)
    }
}

/// # Safety
///
/// `out` holds a count.
#[inline(always)]
pub(crate) unsafe fn write_count(len: usize, out: &mut [u8]) {
    // SAFETY: forwarded; `len` fits a `u32`, checked by `within`.
    unsafe {
        out.as_mut_ptr()
            .cast::<[u8; 4]>()
            .write((len as u32).to_le_bytes())
    };
}
