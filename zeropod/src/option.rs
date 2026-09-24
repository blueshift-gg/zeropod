//! `Option<T>`: a 0 or 1 byte, then the value if 1.

use crate::{__private::from, Error, ZeroPod};

// SAFETY: `check` accepts a tag of 0 alone, or a tag of 1 and a value `T`
// accepts; `len`, `read` and `write` follow the same tag.
unsafe impl<T: ZeroPod> ZeroPod for Option<T> {
    type Ref<'a>
        = Option<T::Ref<'a>>
    where
        Self: 'a;
    type In<'a>
        = Option<T::In<'a>>
    where
        Self: 'a;

    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        match bytes.first() {
            Some(0) => Ok(1),
            Some(1) => Ok(1 + T::check(from(bytes, 1), limits)?),
            Some(_) => Err(Error::InvalidTag),
            None => Err(Error::TooShort),
        }
    }

    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: a valid encoding starts with its tag, and a value follows a
        // tag of 1.
        unsafe {
            match bytes.get_unchecked(0) {
                0 => 1,
                _ => 1 + T::len(bytes.get_unchecked(1..)),
            }
        }
    }

    unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
        // SAFETY: as in `len`.
        unsafe {
            match bytes.get_unchecked(0) {
                0 => None,
                _ => Some(T::read(bytes.get_unchecked(1..))),
            }
        }
    }

    fn encoded_len(value: &Self::In<'_>, limits: &[usize]) -> Result<usize, Error> {
        match value {
            None => Ok(1),
            Some(value) => Ok(1 + T::encoded_len(value, limits)?),
        }
    }

    unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the encoding: the tag, then the value.
        unsafe {
            match value {
                None => {
                    *out.get_unchecked_mut(0) = 0;
                    1
                }
                Some(value) => {
                    *out.get_unchecked_mut(0) = 1;
                    1 + T::write(value, out.get_unchecked_mut(1..))
                }
            }
        }
    }

    fn max_len(limits: &[usize]) -> Option<usize> {
        Some(1 + T::max_len(limits)?)
    }

    fn input(&self) -> Self::In<'_> {
        self.as_ref().map(|value| value.input())
    }

    unsafe fn own(value: Self::Ref<'_>) -> Self {
        // SAFETY: forwarded: the value came from `read`.
        value.map(|value| unsafe { T::own(value) })
    }
}
