//! What derived code calls. Not part of the API.

use crate::{Bytes, Error, ZeroPod};

pub use core;

/// The sum of `sizes`, if they are all known.
pub const fn sum(sizes: &[Option<usize>]) -> Option<usize> {
    let mut total = 0;
    let mut index = 0;
    while index < sizes.len() {
        match sizes[index] {
            Some(size) => total += size,
            None => return None,
        }
        index += 1;
    }
    Some(total)
}

/// The size `sizes` share, if they are all known and equal.
pub const fn same(sizes: &[Option<usize>]) -> Option<usize> {
    let Some(Some(first)) = sizes.first().copied() else {
        return None;
    };
    let mut index = 1;
    while index < sizes.len() {
        match sizes[index] {
            Some(size) if size == first => {}
            _ => return None,
        }
        index += 1;
    }
    Some(first)
}

/// Whether a field of fixed size follows one whose size varies: the fixed
/// ones must come first, at offsets known when compiling.
pub const fn fixed_after_variable(sizes: &[Option<usize>], field: usize) -> bool {
    if sizes[field].is_none() {
        return false;
    }
    let mut index = 0;
    while index < field {
        if sizes[index].is_none() {
            return true;
        }
        index += 1;
    }
    false
}

/// The larger of two bounds, `None` if either is unbounded.
pub fn max(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    Some(a?.max(b?))
}

/// The bytes from `at`, or none if `at` is past the end: the next check
/// then fails as too short.
#[inline(always)]
pub fn from(bytes: &[u8], at: usize) -> &[u8] {
    bytes.get(at..).unwrap_or(&[])
}

/// # Safety
///
/// `at <= bytes.len()`.
#[inline(always)]
pub unsafe fn from_unchecked(bytes: &[u8], at: usize) -> &[u8] {
    // SAFETY: forwarded from this function's contract.
    unsafe { bytes.get_unchecked(at..) }
}

/// # Safety
///
/// `at <= bytes.len()`.
#[inline(always)]
pub unsafe fn from_unchecked_mut(bytes: &mut [u8], at: usize) -> &mut [u8] {
    // SAFETY: forwarded from this function's contract.
    unsafe { bytes.get_unchecked_mut(at..) }
}

/// Replaces the `T` encoded at `at` with `value`, moving the bytes after it
/// up to `total`, the encoding's end, to where the new length puts them.
///
/// # Safety
///
/// `bytes` hold a valid encoding `total` long, in which a field of type `T`
/// with `limits` starts at `at`.
pub unsafe fn replace<T: ZeroPod>(
    bytes: &mut Bytes,
    at: usize,
    total: usize,
    value: &T::In<'_>,
    limits: &[usize],
) -> Result<(), Error> {
    let new = T::encoded_len(value, limits)?;
    // SAFETY: the bytes will hold a valid encoding again: the field is
    // rewritten with a valid value, and the fields after it are moved whole.
    let bytes = unsafe { bytes.as_mut_slice() };
    // SAFETY: a `T` starts at `at`.
    let old = unsafe { T::len(from_unchecked(bytes, at)) };
    let end = total - old + new;
    if end > bytes.len() {
        return Err(Error::NoRoom);
    }
    // From the move to the end of the write the encoding is broken: nothing
    // may unwind out of here and leave the view readable.
    let guard = AbortOnUnwind;
    if new != old {
        // SAFETY: both ranges end within `bytes`: the old one at `total`,
        // the new one at `end`.
        unsafe {
            let base = bytes.as_mut_ptr();
            core::ptr::copy(base.add(at + old), base.add(at + new), total - at - old);
        }
    }
    // SAFETY: `new` bytes are free from `at`: the old field's, or the room
    // just made for it.
    unsafe { T::write(value, from_unchecked_mut(bytes, at)) };
    core::mem::forget(guard);
    Ok(())
}

/// Panics while dropped: dropped by an unwind, which a panic during
/// unwinding turns into an abort.
struct AbortOnUnwind;

impl Drop for AbortOnUnwind {
    fn drop(&mut self) {
        panic!("zeropod: a write unwound with the encoding half moved");
    }
}

/// `T`'s size when it has one, else what `walk` measures.
///
/// # Safety
///
/// `walk` is sound to call on `bytes`.
#[inline(always)]
pub unsafe fn len_of<T: ZeroPod>(bytes: &[u8], walk: impl FnOnce(&[u8]) -> usize) -> usize {
    match T::SIZE {
        Some(size) => size,
        None => walk(bytes),
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Two optional bytes, `[a][b]`: rewriting `a` to any value leaves a
    /// valid encoding with `b` unchanged, or fails with nothing changed.
    #[kani::proof]
    fn replacing_a_field_keeps_the_rest() {
        let mut bytes: [u8; 6] = kani::any();
        let Ok(first) = Option::<u8>::check(&bytes, &[]) else {
            return;
        };
        let Ok(second) = Option::<u8>::check(from(&bytes, first), &[]) else {
            return;
        };
        let total = first + second;
        // SAFETY: checked just above.
        let before = unsafe { Option::<u8>::read(from_unchecked(&bytes, first)) };
        let original = bytes;
        let value: Option<u8> = kani::any();

        // SAFETY: the bytes hold two valid fields, `total` long, the first at 0.
        let result =
            unsafe { replace::<Option<u8>>(Bytes::new_mut(&mut bytes), 0, total, &value, &[]) };

        match result {
            Ok(()) => {
                let first = Option::<u8>::check(&bytes, &[]).unwrap();
                // SAFETY: checked just above.
                assert_eq!(unsafe { Option::<u8>::read(&bytes) }, value);
                let rest = from(&bytes, first);
                assert_eq!(Option::<u8>::check(rest, &[]), Ok(second));
                // SAFETY: checked just above.
                assert_eq!(unsafe { Option::<u8>::read(rest) }, before);
            }
            Err(_) => assert_eq!(bytes, original),
        }
    }
}
