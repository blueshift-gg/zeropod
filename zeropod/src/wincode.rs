//! wincode, for a type shared with code that speaks it: derive its
//! `SchemaWrite` and `SchemaRead` beside `ZeroPod`, and under [`CONFIG`]
//! wincode writes and reads zeropod's bytes.
//!
//! Two types differ under any configuration: `Result`, whose tags wincode
//! swaps, and hash maps and sets, which wincode does not sort. And wincode's
//! `deserialize` leaves trailing bytes be, where `deserialize_exact`, like
//! [`from_slice`](crate::from_slice), rejects them.

use alloc::{string::String, vec, vec::Vec};
use core::mem::MaybeUninit;

use ::wincode::{
    ReadResult, SchemaRead, SchemaWrite, TypeMeta, WriteResult,
    config::{ConfigCore, Configuration, DEFAULT_PREALLOCATION_SIZE_LIMIT},
    containers,
    error::{ReadError, WriteError},
    int_encoding::{FixInt, LittleEndian},
    io::{BorrowKind, Reader, Writer},
    len::{FixIntLen, SeqLen},
};

use crate::{ArrayString, ArrayVec, Plain, Prefix, SmallStr, SmallVec, ZeroPod};

/// [`CONFIG`]'s type.
pub type Config =
    Configuration<true, DEFAULT_PREALLOCATION_SIZE_LIMIT, FixIntLen<u32>, LittleEndian, FixInt, u8>;

/// Borsh's encoding, as wincode configures it: `u32` counts and `u8` tags.
/// Pass it to the functions of `wincode::config`.
pub const CONFIG: Config = Configuration::default()
    .with_length_encoding::<FixIntLen<u32>>()
    .with_tag_encoding::<u8>();

/// The size every `T` encodes to.
const fn size<T: ZeroPod>() -> usize {
    match T::SIZE {
        Some(size) => size,
        None => panic!("an array type has one size"),
    }
}

/// `value`'s encoding, as zeropod writes it.
fn write<T: ZeroPod>(mut writer: impl Writer, value: &T) -> WriteResult<()> {
    // ponytail: one allocation a value; write through a stack buffer if a
    // profile shows it.
    let bytes = crate::to_vec(value).map_err(|_| WriteError::Custom("zeropod: too long"))?;
    writer.write(&bytes)?;
    Ok(())
}

/// A `T` of one size, as zeropod checks and reads it.
fn read<'de, T: ZeroPod>(mut reader: impl Reader<'de>) -> ReadResult<T> {
    let invalid = |_| ReadError::InvalidValue("zeropod: not a valid encoding");
    if reader.supports_borrow(BorrowKind::CallSite) {
        return crate::from_slice(reader.take_scoped(size::<T>())?).map_err(invalid);
    }
    let mut bytes = vec![0; size::<T>()];
    reader.copy_into_slice(&mut bytes)?;
    crate::from_slice(&bytes).map_err(invalid)
}

/// The array types, of one size: zeropod's own encoding, whole.
macro_rules! array {
    ($([$($bounds:tt)*] $ty:ty);* $(;)?) => {$(
        // SAFETY: `write` writes the encoding, which is `SIZE` bytes long.
        unsafe impl<$($bounds)* C: ConfigCore> SchemaWrite<C> for $ty {
            type Src = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: size::<Self>(),
                zero_copy: false,
            };

            fn size_of(_: &Self) -> WriteResult<usize> {
                Ok(size::<Self>())
            }

            fn write(writer: impl Writer, src: &Self) -> WriteResult<()> {
                write(writer, src)
            }
        }

        // SAFETY: `read` takes `SIZE` bytes, and writes `dst` only when it
        // returns `Ok`.
        unsafe impl<'de, $($bounds)* C: ConfigCore> SchemaRead<'de, C> for $ty {
            type Dst = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: size::<Self>(),
                zero_copy: false,
            };

            fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
                dst.write(read(reader)?);
                Ok(())
            }
        }
    )*};
}

array! {
    [const N: usize,] ArrayString<N>;
    [T: Plain + Default, const N: usize,] ArrayVec<T, N>;
}

// SAFETY: wincode's vector after a `P` count, which `size_of` measures and
// `write` writes.
unsafe impl<T, P, C: ConfigCore> SchemaWrite<C> for SmallVec<T, P>
where
    T: SchemaWrite<C, Src = T>,
    P: Prefix,
    FixIntLen<P>: SeqLen<C>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        <containers::Vec<T, FixIntLen<P>> as SchemaWrite<C>>::size_of(src)
    }

    fn write(writer: impl Writer, src: &Self) -> WriteResult<()> {
        <containers::Vec<T, FixIntLen<P>> as SchemaWrite<C>>::write(writer, src)
    }
}

// SAFETY: `read` writes `dst` only when it returns `Ok`.
unsafe impl<'de, T, P, C: ConfigCore> SchemaRead<'de, C> for SmallVec<T, P>
where
    T: SchemaRead<'de, C, Dst = T>,
    P: Prefix,
    FixIntLen<P>: SeqLen<C>,
{
    type Dst = Self;

    fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        dst.write(<containers::Vec<T, FixIntLen<P>> as SchemaRead<C>>::get(reader)?.into());
        Ok(())
    }
}

// SAFETY: wincode's bytes after a `P` count, which `size_of` measures and
// `write` writes.
unsafe impl<P, C: ConfigCore> SchemaWrite<C> for SmallStr<P>
where
    P: Prefix,
    FixIntLen<P>: SeqLen<C>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(<FixIntLen<P> as SeqLen<C>>::write_bytes_needed(src.len())? + src.len())
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        <FixIntLen<P> as SeqLen<C>>::write(writer.by_ref(), src.len())?;
        writer.write(src.as_bytes())?;
        Ok(())
    }
}

// SAFETY: `read` writes `dst` only when it returns `Ok`.
unsafe impl<'de, P, C: ConfigCore> SchemaRead<'de, C> for SmallStr<P>
where
    P: Prefix,
    FixIntLen<P>: SeqLen<C>,
{
    type Dst = Self;

    fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let bytes: Vec<u8> = <containers::Vec<u8, FixIntLen<P>> as SchemaRead<C>>::get(reader)?;
        let text = String::from_utf8(bytes).map_err(|error| error.utf8_error())?;
        dst.write(text.into());
        Ok(())
    }
}
