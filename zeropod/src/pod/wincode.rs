use {
    super::{PodBool, PodOption, PodString, PodVec},
    crate::ZcElem,
    wincode::{config::ConfigCore, TypeMeta},
};

macro_rules! storage_codec {
    ($ty:ty, $error:literal; $($generics:tt)*) => {
        // ZcElem guarantees an initialized, padding-free representation.
        unsafe impl<$($generics)* C: ConfigCore> wincode::SchemaWrite<C> for $ty {
            type Src = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: core::mem::size_of::<Self>(),
                zero_copy: true,
            };

            fn size_of(_: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<Self>())
            }

            fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let bytes = unsafe {
                    core::slice::from_raw_parts(src as *const Self as *const u8, core::mem::size_of::<Self>())
                };
                writer.write(bytes)?;
                Ok(())
            }
        }

        // Every initialized bit pattern is Rust-valid; semantic validation must
        // run before publication, including when nested in another codec.
        unsafe impl<'de, $($generics)* C: ConfigCore> wincode::SchemaRead<'de, C> for $ty {
            type Dst = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: core::mem::size_of::<Self>(),
                zero_copy: false,
            };

            fn read(mut reader: impl wincode::io::Reader<'de>, dst: &mut core::mem::MaybeUninit<Self>) -> wincode::error::ReadResult<()> {
                let bytes = reader.take_scoped(core::mem::size_of::<Self>())?;
                let value = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const Self) };
                <Self as crate::ZcValidate>::validate_ref(&value)
                    .map_err(|_| wincode::error::ReadError::InvalidValue($error))?;
                dst.write(value);
                Ok(())
            }
        }
    };
}

storage_codec!(PodBool, "PodBool validation failed";);
storage_codec!(PodString<N, PFX>, "PodString validation failed"; const N: usize, const PFX: usize,);
storage_codec!(PodVec<T, N, PFX>, "PodVec validation failed"; T: ZcElem, const N: usize, const PFX: usize,);
storage_codec!(PodOption<T, PFX>, "PodOption validation failed"; T: ZcElem, const PFX: usize,);
