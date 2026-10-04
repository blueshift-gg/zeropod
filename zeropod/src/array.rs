//! Arrays: `[T; N]`, `N` items; and `ArrayString<N>` and `ArrayVec<T, N>`, a
//! count then room for `N` bytes or items, full or not. Only the latter two
//! always have a fixed size; native arrays inherit their elements' sizes.

use core::ops::Deref;

use crate::{
    Error, Plain, ZeroPod,
    count::{Prefix, within},
    scalar::item_size,
};

/// Exactly `N` consecutive encodings, borrowed without allocating. Fixed-size
/// elements have constant-time indexing; variable-size elements are walked.
/// Arrays of variable-size elements must follow fixed fields, as usual:
///
/// ```compile_fail
/// #[derive(zeropod::ZeroPod)]
/// struct Misordered {
///     items: [Option<u32>; 2],
///     fixed: u64,
/// }
/// ```
#[repr(transparent)]
pub struct Array<T, const N: usize> {
    item: core::marker::PhantomData<fn() -> T>,
    bytes: [u8],
}

impl<T: ZeroPod, const N: usize> Array<T, N> {
    pub const fn len(&self) -> usize {
        N
    }
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    pub fn get(&self, index: usize) -> Option<T::Ref<'_>> {
        if index >= N {
            return None;
        }
        // SAFETY: this view holds N validated elements and index is in bounds.
        unsafe {
            Some(T::read(
                &self.bytes[element_offset::<T>(&self.bytes, index)..],
            ))
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = T::Ref<'_>> {
        ArrayIter::<T> {
            bytes: &self.bytes,
            left: N,
            item: core::marker::PhantomData,
        }
    }
}

struct ArrayIter<'a, T> {
    bytes: &'a [u8],
    left: usize,
    item: core::marker::PhantomData<fn() -> T>,
}

impl<'a, T: ZeroPod + 'a> Iterator for ArrayIter<'a, T> {
    type Item = T::Ref<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.left = self.left.checked_sub(1)?;
        let bytes = self.bytes;
        // SAFETY: bytes hold `left + 1` validated elements, even for empty
        // encodings. Count elements rather than stopping when bytes run out.
        unsafe {
            self.bytes = &bytes[T::len(bytes)..];
            Some(T::read(bytes))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<'a, T: ZeroPod + 'a> ExactSizeIterator for ArrayIter<'a, T> {}

impl<T: Plain, const N: usize> Deref for Array<T, N> {
    type Target = [T::Stored; N];

    fn deref(&self) -> &Self::Target {
        const { item_size::<T>() };
        // SAFETY: N valid Stored values of alignment 1, including empty arrays.
        unsafe { &*self.bytes.as_ptr().cast() }
    }
}

/// The offset of an element (or the end), within a validated array.
///
/// # Safety
/// `bytes` hold at least `index` valid encodings of T.
pub(crate) unsafe fn element_offset<T: ZeroPod>(bytes: &[u8], index: usize) -> usize {
    match T::SIZE {
        Some(size) => index * size,
        None => (0..index).fold(0, |at, _| {
            // SAFETY: the preceding elements end where this one starts.
            at + unsafe { T::len(&bytes[at..]) }
        }),
    }
}

// SAFETY: every method visits exactly N elements in order, without a count.
// Only Plain's separate Deref casts encodings to their stored representation.
unsafe impl<T: ZeroPod, const N: usize> ZeroPod for [T; N] {
    type Ref<'a>
        = &'a Array<T, N>
    where
        Self: 'a;
    type In<'a>
        = &'a [T; N]
    where
        Self: 'a;
    const SIZE: Option<usize> = if N == 0 {
        Some(0)
    } else {
        match T::SIZE {
            Some(size) => Some(N * size),
            None => None,
        }
    };
    const ANY_BYTES: bool = N == 0 || T::ANY_BYTES;
    const NATIVE: bool = N == 0 || T::NATIVE;

    #[inline]
    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error> {
        if let Some(size) = Self::SIZE {
            if bytes.len() < size {
                return Err(Error::TooShort);
            }
            if Self::ANY_BYTES {
                return Ok(size);
            }
        }
        (0..N).try_fold(0, |at, _| Ok(at + T::check(&bytes[at..], limits)?))
    }

    #[inline]
    unsafe fn len(bytes: &[u8]) -> usize {
        // SAFETY: the encoding contains N valid elements.
        unsafe { element_offset::<T>(bytes, N) }
    }

    #[inline]
    unsafe fn read(bytes: &[u8]) -> &Array<T, N> {
        // SAFETY: Array is transparent over bytes holding N valid elements.
        unsafe { &*(bytes as *const [u8] as *const Array<T, N>) }
    }

    fn encoded_len(values: &&[T; N], limits: &[usize]) -> Result<usize, Error> {
        values.iter().try_fold(0usize, |at, value| {
            at.checked_add(T::encoded_len(&value.input(), limits)?)
                .ok_or(Error::TooLong)
        })
    }

    #[inline]
    unsafe fn write(values: &&[T; N], out: &mut [u8]) -> usize {
        // SAFETY: out holds every preflighted element. NATIVE promises their
        // memory bytes are their encoding; otherwise write each separately.
        unsafe {
            if Self::NATIVE {
                let len = size_of::<[T; N]>();
                core::ptr::copy_nonoverlapping(values.as_ptr().cast::<u8>(), out.as_mut_ptr(), len);
                return len;
            }
            values.iter().fold(0, |at, value| {
                at + T::write(&value.input(), out.get_unchecked_mut(at..))
            })
        }
    }

    fn max_len(limits: &[usize]) -> Option<usize> {
        if N == 0 {
            Some(0)
        } else {
            N.checked_mul(T::max_len(limits)?)
        }
    }

    fn input(&self) -> &[T; N] {
        self
    }

    fn own(items: &Array<T, N>) -> [T; N] {
        let mut iter = items.iter();
        core::array::from_fn(|_| T::own(iter.next().unwrap()))
    }
}

// SAFETY: the items' stored forms, back to back: alignment 1, size `SIZE`,
// and valid exactly when each item is.
unsafe impl<T: Plain, const N: usize> Plain for [T; N] {
    type Stored = [T::Stored; N];

    fn from_stored(stored: &[T::Stored; N]) -> [T; N] {
        stored.map(|item| T::from_stored(&item))
    }
}

/// A string of at most `N` bytes, stored at full capacity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArrayString<const N: usize> {
    len: usize,
    bytes: [u8; N],
}

impl<const N: usize> ArrayString<N> {
    pub const fn new() -> Self {
        Self {
            len: 0,
            bytes: [0; N],
        }
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: the first `len` bytes are UTF-8, as every constructor makes them.
        unsafe { core::str::from_utf8_unchecked(self.bytes.get_unchecked(..self.len)) }
    }
}

impl<const N: usize> Default for ArrayString<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TryFrom<&str> for ArrayString<N> {
    type Error = Error;

    /// Fails with [`Error::TooLong`] past `N` bytes.
    fn try_from(text: &str) -> Result<Self, Error> {
        let mut string = Self::new();
        string
            .bytes
            .get_mut(..text.len())
            .ok_or(Error::TooLong)?
            .copy_from_slice(text.as_bytes());
        string.len = text.len();
        Ok(string)
    }
}

impl<const N: usize> Deref for ArrayString<N> {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> core::fmt::Debug for ArrayString<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}

// SAFETY: `check` accepts a count of at most `N`, then `N` bytes whose first
// `count` are UTF-8; `read` uses those, and `write` writes all `N`.
unsafe impl<const N: usize> ZeroPod for ArrayString<N> {
    type Ref<'a>
        = &'a str
    where
        Self: 'a;
    type In<'a>
        = &'a str
    where
        Self: 'a;
    const SIZE: Option<usize> = Some(4 + N);

    #[inline]
    fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
        let len = u32::count(bytes)?;
        if len > N {
            return Err(Error::TooLong);
        }
        let room = bytes.get(4..4 + N).ok_or(Error::TooShort)?;
        // SAFETY: `len <= N`.
        let text = unsafe { room.get_unchecked(..len) };
        core::str::from_utf8(text).map_err(|_| Error::InvalidUtf8)?;
        Ok(4 + N)
    }

    #[inline]
    unsafe fn len(_: &[u8]) -> usize {
        4 + N
    }

    #[inline]
    unsafe fn read(bytes: &[u8]) -> &str {
        // SAFETY: the count's bytes follow it, and are UTF-8.
        unsafe {
            let len = u32::count_unchecked(bytes);
            core::str::from_utf8_unchecked(bytes.get_unchecked(4..4 + len))
        }
    }

    #[inline]
    fn encoded_len(value: &&str, _: &[usize]) -> Result<usize, Error> {
        within::<u32>(value.len(), &[N])?;
        Ok(4 + N)
    }

    #[inline]
    unsafe fn write(value: &&str, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the count and `N` bytes, and the text fits them.
        unsafe {
            u32::write_count(value.len(), out);
            let room = out.get_unchecked_mut(4..4 + N);
            room.get_unchecked_mut(..value.len())
                .copy_from_slice(value.as_bytes());
            room.get_unchecked_mut(value.len()..).fill(0);
        }
        4 + N
    }

    #[inline]
    fn max_len(_: &[usize]) -> Option<usize> {
        Some(4 + N)
    }

    fn input(&self) -> &str {
        self
    }

    fn own(value: &str) -> Self {
        Self::try_from(value).expect("a string read in place fits its capacity")
    }
}

/// At most `N` items, stored at full capacity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArrayVec<T, const N: usize> {
    len: usize,
    items: [T; N],
}

impl<T: Copy + Default, const N: usize> ArrayVec<T, N> {
    pub fn new() -> Self {
        Self {
            len: 0,
            items: [T::default(); N],
        }
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: `len <= N`, as every constructor keeps it.
        unsafe { self.items.get_unchecked(..self.len) }
    }

    /// Fails with [`Error::TooLong`] when full.
    pub fn push(&mut self, item: T) -> Result<(), Error> {
        *self.items.get_mut(self.len).ok_or(Error::TooLong)? = item;
        self.len += 1;
        Ok(())
    }
}

impl<T: Copy + Default, const N: usize> Default for ArrayVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Default, const N: usize> TryFrom<&[T]> for ArrayVec<T, N> {
    type Error = Error;

    /// Fails with [`Error::TooLong`] past `N` items.
    fn try_from(items: &[T]) -> Result<Self, Error> {
        let mut vec = Self::new();
        vec.items
            .get_mut(..items.len())
            .ok_or(Error::TooLong)?
            .copy_from_slice(items);
        vec.len = items.len();
        Ok(vec)
    }
}

impl<T: Copy + Default, const N: usize> Deref for ArrayVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + Default + core::fmt::Debug, const N: usize> core::fmt::Debug for ArrayVec<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

// SAFETY: `check` accepts a count of at most `N`, then room for `N` items
// whose first `count` are valid; `read` uses those, and `write` writes all
// `N`, zeroing the room past the items.
unsafe impl<T: Plain + Default, const N: usize> ZeroPod for ArrayVec<T, N> {
    type Ref<'a>
        = &'a [T::Stored]
    where
        Self: 'a;
    type In<'a>
        = &'a [T]
    where
        Self: 'a;
    const SIZE: Option<usize> = Some(4 + N * item_size::<T>());

    #[inline]
    fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
        let len = u32::count(bytes)?;
        if len > N {
            return Err(Error::TooLong);
        }
        let size = item_size::<T>();
        let room = bytes.get(4..4 + N * size).ok_or(Error::TooShort)?;
        if !T::ANY_BYTES {
            for index in 0..len {
                // SAFETY: `index < len <= N`, so the item is in the room.
                T::check(unsafe { room.get_unchecked(index * size..) }, &[])?;
            }
        }
        Ok(4 + N * size)
    }

    #[inline]
    unsafe fn len(_: &[u8]) -> usize {
        4 + N * item_size::<T>()
    }

    #[inline]
    unsafe fn read(bytes: &[u8]) -> &[T::Stored] {
        // SAFETY: the count's items follow it, each a valid `Stored` of
        // alignment 1 and size `SIZE`.
        unsafe {
            let len = u32::count_unchecked(bytes);
            core::slice::from_raw_parts(bytes.as_ptr().add(4).cast(), len)
        }
    }

    #[inline]
    fn encoded_len(values: &&[T], _: &[usize]) -> Result<usize, Error> {
        within::<u32>(values.len(), &[N])?;
        Ok(4 + N * item_size::<T>())
    }

    #[inline]
    unsafe fn write(values: &&[T], out: &mut [u8]) -> usize {
        let size = item_size::<T>();
        // SAFETY: `out` holds the count and `N` items, and the values fit them.
        unsafe {
            u32::write_count(values.len(), out);
            let mut at = 4;
            for value in *values {
                at += T::write(&value.input(), out.get_unchecked_mut(at..));
            }
            out.get_unchecked_mut(at..4 + N * size).fill(0);
        }
        4 + N * size
    }

    #[inline]
    fn max_len(_: &[usize]) -> Option<usize> {
        Some(4 + N * item_size::<T>())
    }

    fn input(&self) -> &[T] {
        self
    }

    fn own(items: &[T::Stored]) -> Self {
        assert!(items.len() <= N, "items read in place fit their capacity");
        let mut vec = Self::new();
        for (slot, item) in vec.items.iter_mut().zip(items) {
            *slot = T::from_stored(item);
        }
        vec.len = items.len();
        vec
    }
}
