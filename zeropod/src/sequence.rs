//! `String` and `Vec<T>`: a little-endian `u32` count, then the bytes or
//! the items.

use alloc::{string::String, vec::Vec};
use core::{marker::PhantomData, ops::Deref};

use crate::{
    __private::{from, from_unchecked},
    Error, Plain, ZeroPod,
    count::{count, count_unchecked, within, write_count},
    scalar::item_size,
};

// SAFETY: `check` accepts a count, then that many bytes of UTF-8; `len`,
// `read` and `write` use exactly those.
unsafe impl ZeroPod for String {
    type Ref<'a>
        = &'a str
    where
        Self: 'a;
    type In<'a>
        = &'a str
    where
        Self: 'a;

    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        let len = count(bytes)?;
        within(len, limits)?;
        let end = len.checked_add(4).ok_or(Error::TooShort)?;
        let text = bytes.get(4..end).ok_or(Error::TooShort)?;
        core::str::from_utf8(text).map_err(|_| Error::InvalidUtf8)?;
        Ok(4 + len)
    }

    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: a valid encoding starts with its count.
        4 + unsafe { count_unchecked(bytes) }
    }

    unsafe fn read(bytes: &[u8]) -> &str {
        // SAFETY: the count's bytes follow it, and are UTF-8.
        unsafe {
            let len = count_unchecked(bytes);
            core::str::from_utf8_unchecked(bytes.get_unchecked(4..4 + len))
        }
    }

    fn encoded_len(value: &&str, limits: &[usize]) -> Result<usize, Error> {
        within(value.len(), limits)?;
        Ok(4 + value.len())
    }

    unsafe fn write(value: &&str, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the count and the text.
        unsafe {
            write_count(value.len(), out);
            out.get_unchecked_mut(4..4 + value.len())
                .copy_from_slice(value.as_bytes());
        }
        4 + value.len()
    }

    fn max_len(limits: &[usize]) -> Option<usize> {
        limits.first().map(|limit| 4 + limit)
    }

    fn input(&self) -> &str {
        self
    }

    unsafe fn own(value: &str) -> String {
        value.into()
    }
}

// SAFETY: `check` accepts a count, then that many items `T` accepts; `len`,
// `read` and `write` walk the same items.
unsafe impl<T: ZeroPod> ZeroPod for Vec<T> {
    type Ref<'a>
        = &'a Items<T>
    where
        Self: 'a;
    type In<'a>
        = &'a [T]
    where
        Self: 'a;

    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        let len = count(bytes)?;
        within(len, limits)?;
        match T::SIZE {
            Some(size) if T::ANY_BYTES => {
                let end = len.checked_mul(size).and_then(|items| items.checked_add(4));
                end.filter(|end| *end <= bytes.len()).ok_or(Error::TooShort)
            }
            _ => {
                let inner = limits.get(1..).unwrap_or(&[]);
                let mut at = 4;
                for _ in 0..len {
                    at += T::check(from(bytes, at), inner)?;
                }
                Ok(at)
            }
        }
    }

    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: a valid encoding starts with its count, then its items.
        unsafe {
            let len = count_unchecked(bytes);
            match T::SIZE {
                Some(size) => 4 + len * size,
                None => (0..len).fold(4, |at, _| at + T::len(from_unchecked(bytes, at))),
            }
        }
    }

    unsafe fn read(bytes: &[u8]) -> &Items<T> {
        // SAFETY: `Items` is a transparent `[u8]` holding a valid vector.
        unsafe { &*(bytes as *const [u8] as *const Items<T>) }
    }

    fn encoded_len(values: &&[T], limits: &[usize]) -> Result<usize, Error> {
        within(values.len(), limits)?;
        if let Some(size) = T::SIZE {
            return Ok(4 + values.len() * size);
        }
        let inner = limits.get(1..).unwrap_or(&[]);
        values.iter().try_fold(4, |at, value| {
            Ok(at + T::encoded_len(&value.input(), inner)?)
        })
    }

    unsafe fn write(values: &&[T], out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the count and every item.
        unsafe {
            write_count(values.len(), out);
            if T::NATIVE {
                // A `T`'s bytes are its encoding: the items are one copy.
                let len = size_of_val(*values);
                let items = values.as_ptr().cast::<u8>();
                core::ptr::copy_nonoverlapping(items, out.as_mut_ptr().add(4), len);
                return 4 + len;
            }
            values.iter().fold(4, |at, value| {
                at + T::write(&value.input(), out.get_unchecked_mut(at..))
            })
        }
    }

    fn max_len(limits: &[usize]) -> Option<usize> {
        let (len, inner) = limits.split_first()?;
        len.checked_mul(T::max_len(inner)?)?.checked_add(4)
    }

    fn input(&self) -> &[T] {
        self
    }

    unsafe fn own(items: &Items<T>) -> Vec<T> {
        if T::NATIVE {
            let len = items.len();
            let mut vec = Vec::<T>::with_capacity(len);
            // SAFETY: the items are valid encodings, which are valid `T`s in
            // memory, `size_of::<T>()` each; the vector has room for them.
            unsafe {
                let from = items.bytes.as_ptr().add(4);
                core::ptr::copy_nonoverlapping(
                    from,
                    vec.as_mut_ptr().cast::<u8>(),
                    len * size_of::<T>(),
                );
                vec.set_len(len);
            }
            return vec;
        }
        // SAFETY: forwarded: each item came from `read`.
        items.iter().map(|item| unsafe { T::own(item) }).collect()
    }
}

/// A vector's items, read in place: `len`, `get` and `iter` for any item,
/// and a slice of the stored items for a [`Plain`] one.
#[repr(transparent)]
pub struct Items<T> {
    item: PhantomData<fn() -> T>,
    /// A valid vector: its count, then its items.
    bytes: [u8],
}

impl<T: ZeroPod> Items<T> {
    pub fn len(&self) -> usize {
        // SAFETY: the bytes start with the count.
        unsafe { count_unchecked(&self.bytes) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<T::Ref<'_>> {
        self.iter().nth(index)
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            item: PhantomData,
            at: 4,
            bytes: &self.bytes,
            left: self.len(),
        }
    }
}

impl<T: Plain> Deref for Items<T> {
    type Target = [T::Stored];

    fn deref(&self) -> &[T::Stored] {
        const { item_size::<T>() };
        // SAFETY: the count's items follow it, each a valid `Stored` of
        // alignment 1 and size `SIZE`.
        unsafe { core::slice::from_raw_parts(self.bytes.as_ptr().add(4).cast(), self.len()) }
    }
}

/// The items of an [`Items`], in order.
pub struct Iter<'a, T> {
    item: PhantomData<fn() -> T>,
    bytes: &'a [u8],
    at: usize,
    left: usize,
}

impl<'a, T: ZeroPod + 'a> Iterator for Iter<'a, T> {
    type Item = T::Ref<'a>;

    fn next(&mut self) -> Option<T::Ref<'a>> {
        self.left = self.left.checked_sub(1)?;
        // SAFETY: an item starts at `at`, as `left` counts.
        unsafe {
            let bytes = from_unchecked(self.bytes, self.at);
            self.at += T::len(bytes);
            Some(T::read(bytes))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<'a, T: ZeroPod + 'a> ExactSizeIterator for Iter<'a, T> {}
