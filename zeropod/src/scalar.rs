//! Numbers, `bool`, byte arrays and addresses: one length each.

use crate::{Error, ZeroPod};

/// A type whose encoding has one length and reads in place as `Stored`, so
/// a run of them reads as a `[Stored]`.
///
/// # Safety
///
/// `Stored` has alignment 1, size `SIZE` and no interior mutability, and the
/// bytes `check` accepts are exactly the valid `Stored` values.
pub unsafe trait Plain: ZeroPod + Copy {
    type Stored: Copy + 'static;

    fn from_stored(stored: &Self::Stored) -> Self;
}

/// A [`Plain`] item's size, which it always has, and its stored form's.
pub(crate) const fn item_size<T: Plain>() -> usize {
    match T::SIZE {
        Some(size) if size == size_of::<T::Stored>() => size,
        _ => panic!("a plain type's stored form has its size"),
    }
}

/// Checks that `bytes` hold at least `N` of them.
#[inline(always)]
fn fits<const N: usize>(bytes: &[u8]) -> Result<usize, Error> {
    if bytes.len() >= N {
        Ok(N)
    } else {
        Err(Error::TooShort)
    }
}

macro_rules! number {
    ($($ty:ty => $stored:ty),* $(,)?) => {$(
        // SAFETY: `check` accepts exactly `SIZE` bytes, all of which are a
        // valid number, and `read` and `write` use exactly those.
        unsafe impl ZeroPod for $ty {
            type Ref<'a> = $ty;
            type In<'a> = $ty;
            const SIZE: Option<usize> = Some(size_of::<$ty>());
            const ANY_BYTES: bool = true;
            const NATIVE: bool = cfg!(target_endian = "little");

            #[inline(always)]
            fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
                fits::<{ size_of::<$ty>() }>(bytes)
            }

            #[inline(always)]
            unsafe fn len(_: &[u8]) -> usize {
                size_of::<$ty>()
            }

            #[inline(always)]
            unsafe fn read(bytes: &[u8]) -> $ty {
                // SAFETY: the bytes hold the number, and an array of bytes
                // has alignment 1.
                <$ty>::from_le_bytes(unsafe { *bytes.as_ptr().cast() })
            }

            #[inline(always)]
            fn encoded_len(_: &$ty, _: &[usize]) -> Result<usize, Error> {
                Ok(size_of::<$ty>())
            }

            #[inline(always)]
            unsafe fn write(value: &$ty, out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the number's bytes, and an array of
                // bytes has alignment 1.
                unsafe { out.as_mut_ptr().cast::<[u8; size_of::<$ty>()]>().write(value.to_le_bytes()) };
                size_of::<$ty>()
            }

            fn max_len(_: &[usize]) -> Option<usize> {
                Some(size_of::<$ty>())
            }

            fn input(&self) -> $ty {
                *self
            }

            unsafe fn own(value: $ty) -> $ty {
                value
            }
        }

        // SAFETY: the stored form is the little-endian bytes, of alignment 1,
        // and every one of them is a valid number.
        unsafe impl Plain for $ty {
            type Stored = $stored;

            #[inline(always)]
            fn from_stored(stored: &$stored) -> $ty {
                <$ty>::from(*stored)
            }
        }
    )*};
}

number!(
    u8 => u8, i8 => i8,
    u16 => crate::U16, u32 => crate::U32, u64 => crate::U64, u128 => crate::U128,
    i16 => crate::I16, i32 => crate::I32, i64 => crate::I64, i128 => crate::I128,
);

// SAFETY: `check` accepts one byte, 0 or 1, which is a valid `bool`, and
// `read` and `write` use only it.
unsafe impl ZeroPod for bool {
    type Ref<'a> = bool;
    type In<'a> = bool;
    const SIZE: Option<usize> = Some(1);
    const NATIVE: bool = true;

    #[inline(always)]
    fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
        match bytes.first() {
            Some(0 | 1) => Ok(1),
            Some(_) => Err(Error::InvalidBool),
            None => Err(Error::TooShort),
        }
    }

    #[inline(always)]
    unsafe fn len(_: &[u8]) -> usize {
        1
    }

    #[inline(always)]
    unsafe fn read(bytes: &[u8]) -> bool {
        // SAFETY: the byte is there, and 0 or 1.
        unsafe { *bytes.get_unchecked(0) != 0 }
    }

    #[inline(always)]
    fn encoded_len(_: &bool, _: &[usize]) -> Result<usize, Error> {
        Ok(1)
    }

    #[inline(always)]
    unsafe fn write(value: &bool, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the byte.
        unsafe { *out.get_unchecked_mut(0) = u8::from(*value) };
        1
    }

    fn max_len(_: &[usize]) -> Option<usize> {
        Some(1)
    }

    fn input(&self) -> bool {
        *self
    }

    unsafe fn own(value: bool) -> bool {
        value
    }
}

// SAFETY: a `bool` has alignment 1 and size 1, and `check` accepts exactly
// its valid bytes, 0 and 1.
unsafe impl Plain for bool {
    type Stored = bool;

    fn from_stored(stored: &bool) -> bool {
        *stored
    }
}

/// Bytes of one length, borrowed in place: `[u8; N]` and, with the
/// `solana-address` feature, `Address`.
macro_rules! bytes {
    ($([$($generics:tt)*] $ty:ty, $len:expr);* $(;)?) => {$(
        // SAFETY: `check` accepts `SIZE` bytes, all valid, and `read` and
        // `write` use exactly those.
        unsafe impl<$($generics)*> ZeroPod for $ty {
            type Ref<'a> = &'a $ty;
            type In<'a> = $ty;
            const SIZE: Option<usize> = Some($len);
            const ANY_BYTES: bool = true;
            const NATIVE: bool = true;

            #[inline(always)]
            fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
                fits::<{ $len }>(bytes)
            }

            #[inline(always)]
            unsafe fn len(_: &[u8]) -> usize {
                $len
            }

            #[inline(always)]
            unsafe fn read(bytes: &[u8]) -> &$ty {
                // SAFETY: the bytes are there, and the type is a transparent
                // byte array: alignment 1, every byte valid.
                unsafe { &*bytes.as_ptr().cast::<$ty>() }
            }

            #[inline(always)]
            fn encoded_len(_: &$ty, _: &[usize]) -> Result<usize, Error> {
                Ok($len)
            }

            #[inline(always)]
            unsafe fn write(value: &$ty, out: &mut [u8]) -> usize {
                // SAFETY: as in `read`; `out` holds the bytes.
                unsafe { out.as_mut_ptr().cast::<$ty>().write(*value) };
                $len
            }

            fn max_len(_: &[usize]) -> Option<usize> {
                Some($len)
            }

            fn input(&self) -> $ty {
                *self
            }

            unsafe fn own(value: &$ty) -> $ty {
                *value
            }
        }

        // SAFETY: a transparent byte array: alignment 1, size `SIZE`, every
        // byte valid.
        unsafe impl<$($generics)*> Plain for $ty {
            type Stored = $ty;

            fn from_stored(stored: &$ty) -> $ty {
                *stored
            }
        }
    )*};
}

bytes!([const N: usize] [u8; N], N);

// `Address` is a transparent `[u8; 32]`.
#[cfg(feature = "solana-address")]
const _: () = assert!(
    size_of::<solana_address::Address>() == 32 && align_of::<solana_address::Address>() == 1
);

#[cfg(feature = "solana-address")]
bytes!([] solana_address::Address, 32);
