//! Manual `SchemaWrite` / `SchemaRead` impls for pod types that cannot use
//! derive (generic params, `MaybeUninit` fields).  Each impl serializes the
//! full fixed-size byte representation via raw pointer cast — matching the
//! zero-copy layout used on-chain.

use {
    super::{option::PodOption, string::PodString, vec::PodVec},
    crate::traits::ZcElem,
    wincode::{config::ConfigCore, TypeMeta},
};

const fn static_zero_copy<T>() -> TypeMeta {
    TypeMeta::Static {
        size: core::mem::size_of::<T>(),
        zero_copy: true,
    }
}

// ---------------------------------------------------------------------------
// PodString
// ---------------------------------------------------------------------------

unsafe impl<const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodString<N, PFX>
{
    type Src = Self;

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

unsafe impl<'__de, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
    for PodString<N, PFX>
{
    type Dst = Self;

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

unsafe impl<const N: usize, const PFX: usize, C: ConfigCore> wincode::config::ZeroCopy<C>
    for PodString<N, PFX>
{
}

// ---------------------------------------------------------------------------
// PodVec
// ---------------------------------------------------------------------------

unsafe impl<T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodVec<T, N, PFX>
{
    type Src = Self;

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

unsafe impl<'__de, T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore>
    wincode::SchemaRead<'__de, C> for PodVec<T, N, PFX>
{
    type Dst = Self;

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

unsafe impl<T: ZcElem + 'static, const N: usize, const PFX: usize, C: ConfigCore>
    wincode::config::ZeroCopy<C> for PodVec<T, N, PFX>
{
}

// ---------------------------------------------------------------------------
// PodOption
// ---------------------------------------------------------------------------

unsafe impl<T: Copy, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodOption<T, PFX>
{
    type Src = Self;

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

    const TYPE_META: TypeMeta = static_zero_copy::<Self>();

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

unsafe impl<T: ZcElem + 'static, const PFX: usize, C: ConfigCore> wincode::config::ZeroCopy<C>
    for PodOption<T, PFX>
{
}
