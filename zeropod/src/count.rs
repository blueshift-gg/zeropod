//! The little-endian count strings and vectors start with: Borsh's `u32`,
//! or the `u8` or `u16` of a small string or vector.

use crate::Error;

mod sealed {
    pub trait Sealed {}
}

/// The count a string or vector starts with: `u32`, as Borsh writes it, or
/// the `u8` or `u16` of a [`SmallStr`](crate::SmallStr) or
/// [`SmallVec`](crate::SmallVec).
pub trait Prefix: sealed::Sealed + 'static {
    /// The count's bytes.
    #[doc(hidden)]
    const WIDTH: usize;

    /// The largest count.
    #[doc(hidden)]
    const MAX: usize;

    /// The count `bytes` start with.
    #[doc(hidden)]
    fn count(bytes: &[u8]) -> Result<usize, Error>;

    /// # Safety
    ///
    /// `bytes` start with a count.
    #[doc(hidden)]
    unsafe fn count_unchecked(bytes: &[u8]) -> usize;

    /// # Safety
    ///
    /// `out` holds a count, and `len` is at most `MAX`.
    #[doc(hidden)]
    unsafe fn write_count(len: usize, out: &mut [u8]);
}

macro_rules! prefix {
    ($($ty:ty),*) => {$(
        impl sealed::Sealed for $ty {}

        impl Prefix for $ty {
            const WIDTH: usize = size_of::<$ty>();
            const MAX: usize = <$ty>::MAX as usize;

            #[inline(always)]
            fn count(bytes: &[u8]) -> Result<usize, Error> {
                let count = bytes.first_chunk().ok_or(Error::TooShort)?;
                Ok(<$ty>::from_le_bytes(*count) as usize)
            }

            #[inline(always)]
            unsafe fn count_unchecked(bytes: &[u8]) -> usize {
                // SAFETY: forwarded; an array of bytes has alignment 1.
                <$ty>::from_le_bytes(unsafe { *bytes.as_ptr().cast() }) as usize
            }

            #[inline(always)]
            unsafe fn write_count(len: usize, out: &mut [u8]) {
                // SAFETY: forwarded; `len` fits the count, checked by `within`.
                unsafe {
                    out.as_mut_ptr()
                        .cast::<[u8; size_of::<$ty>()]>()
                        .write((len as $ty).to_le_bytes())
                };
            }
        }
    )*};
}

prefix!(u8, u16, u32);

/// Fails if `len` passes the first of `limits`, or a `P` count.
#[inline(always)]
pub(crate) fn within<P: Prefix>(len: usize, limits: &[usize]) -> Result<(), Error> {
    let limit = limits.first().copied().unwrap_or(P::MAX);
    if len <= limit && len <= P::MAX {
        Ok(())
    } else {
        Err(Error::TooLong)
    }
}

/// The most items `limits` and a `P` count allow, or `None` when only a
/// `u32` bounds them: a length no encoding is expected to reach.
#[cfg(feature = "alloc")]
#[inline(always)]
pub(crate) fn most<P: Prefix>(limits: &[usize]) -> Option<usize> {
    match limits.first() {
        Some(limit) => Some((*limit).min(P::MAX)),
        None => (P::WIDTH < 4).then_some(P::MAX),
    }
}
