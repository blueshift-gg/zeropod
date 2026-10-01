//! What needs an allocator. `String`, `Vec<T>`, maps and sets: a
//! little-endian `u32` count, then the bytes, items or entries, which a map
//! writes sorted by key as Borsh does. `SmallStr` and `SmallVec`: the same
//! after a `u8` or `u16` count. `Box<T>`: its `T`.

use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    __private::{from, from_unchecked},
    Error, Plain, ZeroPod,
    count::{Prefix, most, within},
    scalar::item_size,
};

/// A string whose count is a `u8` or a `u16` rather than Borsh's `u32`: at
/// most 255 or 65,535 bytes, which writing it checks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SmallStr<P = u8>(String, PhantomData<P>);

/// A vector whose count is a `u8` or a `u16` rather than Borsh's `u32`: at
/// most 255 or 65,535 items, which writing it checks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SmallVec<T, P = u8>(Vec<T>, PhantomData<P>);

/// A small type is its heap type with another count: made from it, used as
/// it, and turned back into it.
macro_rules! small {
    ($([$($params:tt)*] $ty:ty => $heap:ty);* $(;)?) => {$(
        impl<$($params)*> From<$heap> for $ty {
            fn from(value: $heap) -> Self {
                Self(value, PhantomData)
            }
        }

        impl<$($params)*> From<$ty> for $heap {
            fn from(value: $ty) -> Self {
                value.0
            }
        }

        impl<$($params)*> Deref for $ty {
            type Target = $heap;

            fn deref(&self) -> &$heap {
                &self.0
            }
        }

        impl<$($params)*> DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut $heap {
                &mut self.0
            }
        }
    )*};
}

small! {
    [P] SmallStr<P> => String;
    [T, P] SmallVec<T, P> => Vec<T>;
}

impl<P> From<&str> for SmallStr<P> {
    fn from(text: &str) -> Self {
        String::from(text).into()
    }
}

/// Strings: a `$count`, then that many bytes of UTF-8.
macro_rules! string {
    ($([$($bounds:tt)*] $ty:ty, $count:ty);* $(;)?) => {$(
        // SAFETY: `check` accepts a count, then that many bytes of UTF-8;
        // `len`, `read` and `write` use exactly those.
        unsafe impl<$($bounds)*> ZeroPod for $ty {
            type Ref<'a>
                = &'a str
            where
                Self: 'a;
            type In<'a>
                = &'a str
            where
                Self: 'a;

            #[inline]
            fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
                let len = <$count>::count(bytes)?;
                within::<$count>(len, limits)?;
                let end = len.checked_add(<$count>::WIDTH).ok_or(Error::TooShort)?;
                let text = bytes.get(<$count>::WIDTH..end).ok_or(Error::TooShort)?;
                core::str::from_utf8(text).map_err(|_| Error::InvalidUtf8)?;
                Ok(end)
            }

            #[inline]
            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: a valid encoding starts with its count.
                <$count>::WIDTH + unsafe { <$count>::count_unchecked(bytes) }
            }

            #[inline]
            unsafe fn read(bytes: &[u8]) -> &str {
                // SAFETY: the count's bytes follow it, and are UTF-8.
                unsafe {
                    let len = <$count>::count_unchecked(bytes);
                    core::str::from_utf8_unchecked(
                        bytes.get_unchecked(<$count>::WIDTH..<$count>::WIDTH + len),
                    )
                }
            }

            #[inline]
            fn encoded_len(value: &&str, limits: &[usize]) -> Result<usize, Error> {
                within::<$count>(value.len(), limits)?;
                Ok(<$count>::WIDTH + value.len())
            }

            #[inline]
            unsafe fn write(value: &&str, out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the count and the text.
                unsafe {
                    <$count>::write_count(value.len(), out);
                    out.get_unchecked_mut(<$count>::WIDTH..<$count>::WIDTH + value.len())
                        .copy_from_slice(value.as_bytes());
                }
                <$count>::WIDTH + value.len()
            }

            #[inline]
            fn max_len(limits: &[usize]) -> Option<usize> {
                most::<$count>(limits).map(|most| <$count>::WIDTH + most)
            }

            fn input(&self) -> &str {
                self
            }

            fn own(value: &str) -> Self {
                value.into()
            }
        }
    )*};
}

string! {
    [] String, u32;
    [P: Prefix] SmallStr<P>, P;
}

/// Vectors: a `$count`, then that many items.
macro_rules! vector {
    ($([$($bounds:tt)*] $ty:ty, $count:ty);* $(;)?) => {$(
        // SAFETY: `check` accepts a count, then that many items `T` accepts;
        // `len`, `read` and `write` walk the same items.
        unsafe impl<$($bounds)*> ZeroPod for $ty {
            type Ref<'a>
                = &'a Items<T, $count>
            where
                Self: 'a;
            type In<'a>
                = &'a [T]
            where
                Self: 'a;

            #[inline]
            fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
                let len = <$count>::count(bytes)?;
                within::<$count>(len, limits)?;
                match T::SIZE {
                    Some(size) if T::ANY_BYTES => {
                        let end = len
                            .checked_mul(size)
                            .and_then(|items| items.checked_add(<$count>::WIDTH));
                        end.filter(|end| *end <= bytes.len()).ok_or(Error::TooShort)
                    }
                    _ => {
                        let inner = limits.get(1..).unwrap_or(&[]);
                        let mut at = <$count>::WIDTH;
                        for _ in 0..len {
                            at += T::check(from(bytes, at), inner)?;
                        }
                        Ok(at)
                    }
                }
            }

            #[inline]
            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: a valid encoding starts with its count, then its items.
                unsafe {
                    let len = <$count>::count_unchecked(bytes);
                    match T::SIZE {
                        Some(size) => <$count>::WIDTH + len * size,
                        None => (0..len).fold(<$count>::WIDTH, |at, _| {
                            at + T::len(from_unchecked(bytes, at))
                        }),
                    }
                }
            }

            #[inline]
            unsafe fn read(bytes: &[u8]) -> &Items<T, $count> {
                // SAFETY: `Items` is a transparent `[u8]` holding a valid vector.
                unsafe { &*(bytes as *const [u8] as *const Items<T, $count>) }
            }

            #[inline]
            fn encoded_len(values: &&[T], limits: &[usize]) -> Result<usize, Error> {
                within::<$count>(values.len(), limits)?;
                if let Some(size) = T::SIZE {
                    return Ok(<$count>::WIDTH + values.len() * size);
                }
                let inner = limits.get(1..).unwrap_or(&[]);
                values.iter().try_fold(<$count>::WIDTH, |at, value| {
                    Ok(at + T::encoded_len(&value.input(), inner)?)
                })
            }

            #[inline]
            unsafe fn write(values: &&[T], out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the count and every item.
                unsafe {
                    <$count>::write_count(values.len(), out);
                    if T::NATIVE {
                        // A `T`'s bytes are its encoding: the items are one copy.
                        let len = size_of_val(*values);
                        let items = values.as_ptr().cast::<u8>();
                        let to = out.as_mut_ptr().add(<$count>::WIDTH);
                        core::ptr::copy_nonoverlapping(items, to, len);
                        return <$count>::WIDTH + len;
                    }
                    values.iter().fold(<$count>::WIDTH, |at, value| {
                        at + T::write(&value.input(), out.get_unchecked_mut(at..))
                    })
                }
            }

            #[inline]
            fn max_len(limits: &[usize]) -> Option<usize> {
                let inner = limits.get(1..).unwrap_or(&[]);
                most::<$count>(limits)?
                    .checked_mul(T::max_len(inner)?)?
                    .checked_add(<$count>::WIDTH)
            }

            fn input(&self) -> &[T] {
                self
            }

            fn own(items: &Items<T, $count>) -> Self {
                items.to_vec().into()
            }
        }
    )*};
}

vector! {
    [T: ZeroPod] Vec<T>, u32;
    [T: ZeroPod, P: Prefix] SmallVec<T, P>, P;
}

/// A vector's items, read in place: `len`, `get` and `iter` for any item,
/// and a slice of the stored items for a [`Plain`] one. `P` is the count
/// they follow.
#[repr(transparent)]
pub struct Items<T, P = u32> {
    item: PhantomData<fn() -> (T, P)>,
    /// A valid vector: its count, then its items.
    bytes: [u8],
}

impl<T: ZeroPod, P: Prefix> Items<T, P> {
    pub fn len(&self) -> usize {
        // SAFETY: the bytes start with the count.
        unsafe { P::count_unchecked(&self.bytes) }
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
            at: P::WIDTH,
            bytes: &self.bytes,
            left: self.len(),
        }
    }

    /// The items, owned.
    fn to_vec(&self) -> Vec<T> {
        if T::NATIVE {
            let len = self.len();
            let mut vec = Vec::<T>::with_capacity(len);
            // SAFETY: the items are valid encodings, which are valid `T`s in
            // memory, `size_of::<T>()` each; the vector has room for them.
            unsafe {
                let from = self.bytes.as_ptr().add(P::WIDTH);
                core::ptr::copy_nonoverlapping(
                    from,
                    vec.as_mut_ptr().cast::<u8>(),
                    len * size_of::<T>(),
                );
                vec.set_len(len);
            }
            return vec;
        }
        self.iter().map(|item| T::own(item)).collect()
    }
}

impl<T: Plain, P: Prefix> Deref for Items<T, P> {
    type Target = [T::Stored];

    fn deref(&self) -> &[T::Stored] {
        const { item_size::<T>() };
        // SAFETY: the count's items follow it, each a valid `Stored` of
        // alignment 1 and size `SIZE`.
        unsafe { core::slice::from_raw_parts(self.bytes.as_ptr().add(P::WIDTH).cast(), self.len()) }
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

// SAFETY: every method is `T`'s.
unsafe impl<T: ZeroPod> ZeroPod for Box<T> {
    type Ref<'a>
        = T::Ref<'a>
    where
        Self: 'a;
    type In<'a>
        = T::In<'a>
    where
        Self: 'a;
    const SIZE: Option<usize> = T::SIZE;
    const ANY_BYTES: bool = T::ANY_BYTES;

    #[inline]
    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        T::check(bytes, limits)
    }

    #[inline]
    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: forwarded.
        unsafe { T::len(bytes) }
    }

    #[inline]
    unsafe fn read(bytes: &[u8]) -> T::Ref<'_> {
        // SAFETY: forwarded.
        unsafe { T::read(bytes) }
    }

    #[inline]
    fn encoded_len(value: &T::In<'_>, limits: &[usize]) -> Result<usize, Error> {
        T::encoded_len(value, limits)
    }

    #[inline]
    unsafe fn write(value: &T::In<'_>, out: &mut [u8]) -> usize {
        // SAFETY: forwarded.
        unsafe { T::write(value, out) }
    }

    #[inline]
    fn max_len(limits: &[usize]) -> Option<usize> {
        T::max_len(limits)
    }

    fn input(&self) -> T::In<'_> {
        (**self).input()
    }

    fn own(value: T::Ref<'_>) -> Self {
        Box::new(T::own(value))
    }
}

/// Maps and sets, whose encoding is a vector of their entries in key order:
/// they check and read as that vector does, and write their entries sorted.
macro_rules! collection {
    ($([$($bounds:tt)*] $ty:ty, $entry:ty, $entries:ident => $sorted:expr, $pair:expr, $owned:expr);* $(;)?) => {$(
        // SAFETY: the encoding is `Vec<$entry>`'s, whose methods check, measure
        // and read it; `write` writes the count, then each entry.
        unsafe impl<$($bounds)*> ZeroPod for $ty {
            type Ref<'a>
                = &'a Items<$entry>
            where
                Self: 'a;
            type In<'a>
                = &'a $ty
            where
                Self: 'a;

            #[inline]
            fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
                <Vec<$entry> as ZeroPod>::check(bytes, limits)
            }

            #[inline]
            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: forwarded.
                unsafe { <Vec<$entry> as ZeroPod>::len(bytes) }
            }

            #[inline]
            unsafe fn read(bytes: &[u8]) -> &Items<$entry> {
                // SAFETY: forwarded.
                unsafe { <Vec<$entry> as ZeroPod>::read(bytes) }
            }

            #[inline]
            fn encoded_len($entries: &&$ty, limits: &[usize]) -> Result<usize, Error> {
                within::<u32>($entries.len(), limits)?;
                let entry = |entry| <$entry>::encoded_len(&entry, &[]);
                $entries.iter().map($pair).try_fold(4, |at, pair| Ok(at + entry(pair)?))
            }

            #[inline]
            unsafe fn write($entries: &&$ty, out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the count and every entry.
                unsafe {
                    u32::write_count($entries.len(), out);
                    $sorted.into_iter().map($pair).fold(4, |at, pair| {
                        at + <$entry>::write(&pair, out.get_unchecked_mut(at..))
                    })
                }
            }


            #[inline]
            fn max_len(limits: &[usize]) -> Option<usize> {
                <Vec<$entry> as ZeroPod>::max_len(limits)
            }

            fn input(&self) -> &$ty {
                self
            }

            fn own(items: &Items<$entry>) -> $ty {
                items.iter().map(|entry| <$entry>::own(entry)).map($owned).collect()
            }
        }
    )*};
}

collection! {
    [K: ZeroPod + Ord, V: ZeroPod] BTreeMap<K, V>, (K, V), map =>
        map.iter(), |(key, value)| (key.input(), value.input()), |(key, value)| (key, value);
    [K: ZeroPod + Ord] BTreeSet<K>, K, set =>
        set.iter(), |key: &K| key.input(), |key| key;
}

#[cfg(feature = "std")]
mod hashed {
    use std::{
        collections::{HashMap, HashSet},
        hash::{BuildHasher, Hash},
    };

    use super::*;

    /// A map's or set's entries by key, as Borsh writes them.
    fn sorted<T, K: Ord>(entries: impl Iterator<Item = T>, key: impl Fn(&T) -> &K) -> Vec<T> {
        let mut entries: Vec<T> = entries.collect();
        entries.sort_unstable_by(|a, b| key(a).cmp(key(b)));
        entries
    }

    collection! {
        [K: ZeroPod + Ord + Hash, V: ZeroPod, S: BuildHasher + Default] HashMap<K, V, S>, (K, V), map =>
            sorted(map.iter(), |(key, _)| *key), |(key, value)| (key.input(), value.input()), |(key, value)| (key, value);
        [K: ZeroPod + Ord + Hash, S: BuildHasher + Default] HashSet<K, S>, K, set =>
            sorted(set.iter(), |key| *key), |key: &K| key.input(), |key| key;
    }
}
