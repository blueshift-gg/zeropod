//! Borsh's encoding, read and written in place.
//!
//! Derive [`ZeroPod`] where you would derive `BorshSerialize` and
//! `BorshDeserialize`: the bytes are the same. Owned values go through
//! [`to_vec`] and [`from_slice`]; on chain, [`Layout::view`] validates the
//! bytes once and reads each field where it lies, and [`Layout::view_mut`]
//! writes fields in place.
//!
//! ```
//! use zeropod::{Layout, ZeroPod};
//!
//! #[derive(ZeroPod, Debug, PartialEq)]
//! struct Profile {
//!     score: u64,
//!     #[max_len(32)]
//!     name: String,
//! }
//!
//! let mut bytes = zeropod::to_vec(&Profile { score: 7, name: "ada".into() }).unwrap();
//! bytes.resize(64, 0); // room to grow
//! let profile = Profile::view_mut(&mut bytes).unwrap();
//! assert_eq!((profile.score(), profile.name()), (7, "ada"));
//! profile.set_name("lovelace").unwrap();
//! assert_eq!(profile.size(), 8 + 4 + 8);
//! ```
//!
//! # Layout
//!
//! Fields of fixed size come first, at offsets known when compiling; fields
//! whose size varies (`String`, `Vec`, `Option`, enums whose variants differ
//! in size) come last, which does not compile otherwise:
//!
//! ```compile_fail
//! #[derive(zeropod::ZeroPod)]
//! struct Misordered {
//!     #[max_len(8)]
//!     name: String,
//!     score: u64, // `score` has a fixed size, so it comes before ...
//! }
//! ```
//!
//! [`ArrayString`] and [`ArrayVec`] store a string or a vector at full
//! capacity, so they are of fixed size, and a struct of fixed fields only has
//! one size.
//! [`SmallStr`] and [`SmallVec`] are a `String` and a `Vec` after a `u8` or
//! a `u16` count instead of Borsh's `u32`.
//!
//! # Your own types
//!
//! Derive `ZeroPod` on any struct or enum of storable fields, generic or
//! not. A struct of one unnamed field is stored as that field: `struct
//! Lamports(u64)` is a `u64` on the wire, and with `#[repr(transparent)]` a
//! vector of them is copied whole. Borsh's `#[borsh(skip)]` and
//! `#[borsh(use_discriminant = ..)]` mean what they mean to Borsh.
//!
//! A type the derive cannot express implements [`ZeroPod`] by hand: an
//! `unsafe` trait, whose contract its documentation spells out, and whose
//! implementations in this crate, one per wire type, are the examples.
//!
//! # Safety
//!
//! A view is a [`Bytes`], whose bytes only this crate can change, and only
//! to another valid encoding: once validated, a view stays valid. Even in the
//! module that declares the type, the byte mutation method is private:
//!
//! ```compile_fail
//! # use zeropod::{Layout, ZeroPod};
//! #[derive(ZeroPod)]
//! struct Named {
//!     #[max_len(8)]
//!     name: String,
//! }
//!
//! let mut bytes = zeropod::to_vec(&Named { name: "ada".into() }).unwrap();
//! let view = Named::view_mut(&mut bytes).unwrap();
//! unsafe { view.1.as_mut_slice()[4] = 0xff; } // private, even in an unsafe block
//! ```
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod array;
mod bytes;
mod count;
mod edit;
mod error;
#[cfg(feature = "alloc")]
mod heap;
mod scalar;
mod tagged;
mod tuple;
#[cfg(feature = "wincode")]
pub mod wincode;
mod zc;

#[doc(hidden)]
pub mod __private;

pub use array::{Array, ArrayString, ArrayVec, Iter};
pub use bytes::Bytes;
pub use count::Prefix;
pub use edit::{Edit, EditFields};
pub use error::Error;
#[cfg(feature = "alloc")]
pub use heap::{Items, SmallStr, SmallVec};
pub use scalar::Plain;
pub use zc::{ZcElem, ZcValidate};
pub use zerocopy::little_endian::{I16, I32, I64, I128, U16, U32, U64, U128};
pub use zeropod_derive::ZeroPod;

/// A type stored in Borsh's encoding and read in place.
///
/// # Safety
///
/// Views and setters rely on these, which implementors promise:
///
/// - `check` depends only on its arguments. When it accepts `bytes` with
///   `limits`, returning `n`, then `len` returns `n`, and `read` touches only
///   those `n` bytes and returns a value safe code may use.
/// - When `encoded_len(value, limits)` returns `Ok(n)` and `out` holds `n`
///   bytes, `write` writes exactly `out[..n]`, returns `n`, and `check`
///   accepts the result with the same `limits`.
/// - `write` and `input` do not unwind.
/// - `SIZE`, when set, is the length of every encoding; `ANY_BYTES`, when
///   set, means every `SIZE` bytes are a valid encoding; `NATIVE`, when set,
///   means a value's bytes in memory are its encoding, `SIZE` long.
/// - The safe methods are sound for any arguments.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be stored by zeropod",
    label = "use an integer, `bool`, a byte array, `String`, `Vec`, `Option`, an array type, or a type deriving `ZeroPod`"
)]
pub unsafe trait ZeroPod: Sized {
    /// What a view reads: a copy of a number, a reference to text, bytes or
    /// items, or the view of a nested value.
    type Ref<'a>
    where
        Self: 'a;

    /// What a view writes.
    type In<'a>
    where
        Self: 'a;

    /// The length of every encoding, if they all have one.
    const SIZE: Option<usize> = None;

    /// Whether every `SIZE` bytes are a valid encoding, so checking a run of
    /// them is checking its length.
    const ANY_BYTES: bool = false;

    /// Whether a value's bytes in memory are its encoding, so a run of them
    /// is written or read with one copy.
    const NATIVE: bool = false;

    /// Checks that `bytes` start with a valid encoding within `limits`, the
    /// `#[max_len]` bounds from the outermost in, and returns its length.
    fn check(bytes: &[u8], limits: &[usize]) -> Result<usize, Error>;

    /// The length of the encoding `bytes` start with.
    ///
    /// # Safety
    ///
    /// `bytes` start with an encoding `check` accepts.
    unsafe fn len(bytes: &[u8]) -> usize;

    /// The value `bytes` start with.
    ///
    /// # Safety
    ///
    /// As for [`len`](Self::len).
    unsafe fn read(bytes: &[u8]) -> Self::Ref<'_>;

    /// The length `value` encodes to, or [`Error::TooLong`] past `limits`.
    fn encoded_len(value: &Self::In<'_>, limits: &[usize]) -> Result<usize, Error>;

    /// Writes `value`'s encoding at the start of `out`, and returns its length.
    ///
    /// # Safety
    ///
    /// `out` holds at least `encoded_len(value, ..)` bytes.
    unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize;

    /// The longest encoding within `limits`, or `None` if one is unbounded.
    fn max_len(limits: &[usize]) -> Option<usize>;

    /// The value as a view would write it.
    fn input(&self) -> Self::In<'_>;

    /// The owned value of what a view read. Sound for any argument, which a
    /// caller may build itself: an enum's `Ref`, say.
    fn own(value: Self::Ref<'_>) -> Self;
}

/// A struct deriving [`ZeroPod`], read and written through its view.
///
/// # Safety
///
/// `View` is a `#[repr(transparent)]` [`Bytes`] whose `as_ref` returns it,
/// and `view_unchecked` and `view_unchecked_mut` cast the bytes to it.
/// Setters trust [`size`](Self::size) through these.
pub unsafe trait Layout: ZeroPod {
    /// A [`Bytes`] holding a valid encoding, with a method per field.
    type View: ?Sized + AsRef<Bytes>;

    /// Validates `bytes`, which may run on past the encoding, and views them.
    fn view(bytes: &[u8]) -> Result<&Self::View, Error> {
        Self::check(bytes, &[])?;
        // SAFETY: `bytes` start with a valid encoding.
        Ok(unsafe { Self::view_unchecked(bytes) })
    }

    /// Validates `bytes` and views them to write. Bytes past the encoding are
    /// room a field may grow into.
    fn view_mut(bytes: &mut [u8]) -> Result<&mut Self::View, Error> {
        Self::check(bytes, &[])?;
        // SAFETY: as in `view`.
        Ok(unsafe { Self::view_unchecked_mut(bytes) })
    }

    /// # Safety
    ///
    /// `bytes` start with an encoding `check` accepts.
    unsafe fn view_unchecked(bytes: &[u8]) -> &Self::View;

    /// # Safety
    ///
    /// As for [`view_unchecked`](Self::view_unchecked).
    unsafe fn view_unchecked_mut(bytes: &mut [u8]) -> &mut Self::View;

    /// The bytes a view's encoding takes, without the room after it.
    fn size(view: &Self::View) -> usize {
        // SAFETY: a view holds a valid encoding.
        unsafe { Self::len(view.as_ref().as_slice()) }
    }
}

/// `value`'s encoding: `borsh::to_vec`.
#[cfg(feature = "alloc")]
pub fn to_vec<T: ZeroPod>(value: &T) -> Result<alloc::vec::Vec<u8>, Error> {
    let input = value.input();
    let mut bytes = alloc::vec![0; T::encoded_len(&input, &[])?];
    // SAFETY: `bytes` is exactly the encoded length.
    unsafe { T::write(&input, &mut bytes) };
    Ok(bytes)
}

/// Writes `value`'s encoding at the start of `out`, and returns its length.
pub fn write<T: ZeroPod>(value: &T, out: &mut [u8]) -> Result<usize, Error> {
    let input = value.input();
    let len = T::encoded_len(&input, &[])?;
    if len > out.len() {
        return Err(Error::NoRoom);
    }
    // SAFETY: `out` holds at least `len` bytes.
    unsafe { T::write(&input, out) };
    Ok(len)
}

/// Validates `bytes`, which may run on past the encoding, and reads the
/// value in place: a struct's view, an enum's reference, a string, ...
pub fn read<T: ZeroPod>(bytes: &[u8]) -> Result<T::Ref<'_>, Error> {
    T::check(bytes, &[])?;
    // SAFETY: `bytes` start with a valid encoding.
    Ok(unsafe { T::read(bytes) })
}

/// The value `bytes` encode, all of them: `borsh::from_slice`.
pub fn from_slice<T: ZeroPod>(bytes: &[u8]) -> Result<T, Error> {
    if T::check(bytes, &[])? != bytes.len() {
        return Err(Error::TrailingBytes);
    }
    // SAFETY: `bytes` are a valid encoding.
    Ok(T::own(unsafe { T::read(bytes) }))
}
