//! Manual `SchemaWrite` / `SchemaRead` impls for pod types that cannot use
//! derive (generic params, `MaybeUninit` fields).  Each impl serializes the
//! full fixed-size byte representation via raw pointer cast — matching the
//! zero-copy layout used on-chain.
//!
//! `PodString` and `PodVec` keep their dynamic data in a `[MaybeUninit<_>; N]`
//! capacity array: only the length prefix and the first `len` bytes are
//! initialized, the rest of the capacity is uninitialized. Serializing them by
//! casting the whole struct to `&[u8]` would read that uninitialized capacity,
//! which is undefined behavior and also leaks stale stack/heap bytes into the
//! output (making it non-deterministic). Instead we write the initialized
//! prefix + active bytes and zero-fill the remaining capacity, so the on-wire
//! image stays a fixed `size_of::<Self>()` bytes but is fully initialized and
//! deterministic.

use {
    super::{option::PodOption, string::PodString, vec::PodVec},
    crate::traits::ZcElem,
    wincode::config::ConfigCore,
};

/// Write `n` zero bytes without allocating, used to pad a fixed-size pod image
/// out to its declared `size_of` once the initialized bytes have been written.
#[inline]
fn write_zeroed_padding(
    mut writer: impl wincode::io::Writer,
    mut n: usize,
) -> wincode::error::WriteResult<()> {
    const ZEROS: [u8; 64] = [0u8; 64];
    while n > 0 {
        let chunk = n.min(ZEROS.len());
        writer.write(&ZEROS[..chunk])?;
        n -= chunk;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// PodString
// ---------------------------------------------------------------------------

unsafe impl<const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodString<N, PFX>
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        // Initialized region is the PFX-byte length prefix followed by the
        // first `len` data bytes; both live contiguously at the front of the
        // (align-1, padding-free) struct.
        let __init_len = PFX + src.len();
        // SAFETY: `src` is `#[repr(C)]` align 1, so the length prefix occupies
        // bytes `[0, PFX)` and the active data `[PFX, PFX + len)`, all
        // initialized and contiguous. `len()` is clamped to `N`, so
        // `__init_len <= size_of::<Self>()`.
        let __init =
            unsafe { core::slice::from_raw_parts(src as *const Self as *const u8, __init_len) };
        __writer.write(__init)?;
        write_zeroed_padding(__writer.by_ref(), core::mem::size_of::<Self>() - __init_len)?;
        Ok(())
    }
}

unsafe impl<'__de, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
    for PodString<N, PFX>
{
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodString validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodVec
// ---------------------------------------------------------------------------

unsafe impl<T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodVec<T, N, PFX>
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        // Initialized region is the PFX-byte length prefix followed by the
        // first `len` elements. `T: ZcElem` is align 1, so elements are packed
        // with no padding and sit contiguously after the prefix.
        let __init_len = PFX + src.len() * core::mem::size_of::<T>();
        // SAFETY: `src` is `#[repr(C)]` align 1; the prefix occupies `[0, PFX)`
        // and the active elements `[PFX, PFX + len * size_of::<T>())`, all
        // initialized and contiguous. `len()` is clamped to `N`, so
        // `__init_len <= size_of::<Self>()`.
        let __init =
            unsafe { core::slice::from_raw_parts(src as *const Self as *const u8, __init_len) };
        __writer.write(__init)?;
        write_zeroed_padding(__writer.by_ref(), core::mem::size_of::<Self>() - __init_len)?;
        Ok(())
    }
}

unsafe impl<'__de, T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore>
    wincode::SchemaRead<'__de, C> for PodVec<T, N, PFX>
{
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodVec validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodOption
// ---------------------------------------------------------------------------

unsafe impl<T: Copy, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodOption<T, PFX>
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let __bytes = unsafe {
            core::slice::from_raw_parts(
                src as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        __writer.write(__bytes)?;
        Ok(())
    }
}

unsafe impl<'__de, T: ZcElem, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
    for PodOption<T, PFX>
{
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodOption validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}
