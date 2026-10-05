//! `Option<T>`, a 0 or 1 byte then the value if 1; and `Result<T, E>`, 0
//! then the error or 1 then the value, as Borsh writes them.

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

    #[inline]
    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        match bytes.first() {
            Some(0) => Ok(1),
            Some(1) => Ok(1 + T::check(from(bytes, 1), limits)?),
            Some(_) => Err(Error::InvalidTag),
            None => Err(Error::TooShort),
        }
    }

    #[inline]
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

    #[inline]
    unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
        // SAFETY: as in `len`.
        unsafe {
            match bytes.get_unchecked(0) {
                0 => None,
                _ => Some(T::read(bytes.get_unchecked(1..))),
            }
        }
    }

    #[inline]
    fn encoded_len(value: &Self::In<'_>, limits: &[usize]) -> Result<usize, Error> {
        match value {
            None => Ok(1),
            Some(value) => Ok(1 + T::encoded_len(value, limits)?),
        }
    }

    #[inline]
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

    #[inline]
    fn max_len(limits: &[usize]) -> Option<usize> {
        Some(1 + T::max_len(limits)?)
    }

    fn input(&self) -> Self::In<'_> {
        self.as_ref().map(|value| value.input())
    }

    fn own(value: Self::Ref<'_>) -> Self {
        value.map(|value| T::own(value))
    }
}

// SAFETY: `check` accepts a tag of 0 and an `E`, or of 1 and a `T`; `len`,
// `read` and `write` follow the same tag.
unsafe impl<T: ZeroPod, E: ZeroPod> ZeroPod for Result<T, E> {
    type Ref<'a>
        = Result<T::Ref<'a>, E::Ref<'a>>
    where
        Self: 'a;
    type In<'a>
        = Result<T::In<'a>, E::In<'a>>
    where
        Self: 'a;

    #[inline]
    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        match bytes.first() {
            Some(0) => Ok(1 + E::check(from(bytes, 1), limits)?),
            Some(1) => Ok(1 + T::check(from(bytes, 1), limits)?),
            Some(_) => Err(Error::InvalidTag),
            None => Err(Error::TooShort),
        }
    }

    #[inline]
    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: a valid encoding starts with its tag, then its value.
        unsafe {
            let value = bytes.get_unchecked(1..);
            match bytes.get_unchecked(0) {
                0 => 1 + E::len(value),
                _ => 1 + T::len(value),
            }
        }
    }

    #[inline]
    unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
        // SAFETY: as in `len`.
        unsafe {
            let value = bytes.get_unchecked(1..);
            match bytes.get_unchecked(0) {
                0 => Err(E::read(value)),
                _ => Ok(T::read(value)),
            }
        }
    }

    #[inline]
    fn encoded_len(value: &Self::In<'_>, limits: &[usize]) -> Result<usize, Error> {
        match value {
            Err(error) => Ok(1 + E::encoded_len(error, limits)?),
            Ok(value) => Ok(1 + T::encoded_len(value, limits)?),
        }
    }

    #[inline]
    unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the encoding: the tag, then the value.
        unsafe {
            let (tag, rest) = out.split_at_mut_unchecked(1);
            match value {
                Err(error) => {
                    tag[0] = 0;
                    1 + E::write(error, rest)
                }
                Ok(value) => {
                    tag[0] = 1;
                    1 + T::write(value, rest)
                }
            }
        }
    }

    #[inline]
    fn max_len(limits: &[usize]) -> Option<usize> {
        Some(1 + T::max_len(limits)?.max(E::max_len(limits)?))
    }

    fn input(&self) -> Self::In<'_> {
        self.as_ref()
            .map(|value| value.input())
            .map_err(|error| error.input())
    }

    fn own(value: Self::Ref<'_>) -> Self {
        value
            .map(|value| T::own(value))
            .map_err(|error| E::own(error))
    }
}
