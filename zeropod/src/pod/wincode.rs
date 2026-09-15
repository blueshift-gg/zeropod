use {
    super::{PodBool, PodOption, PodString, PodVec},
    crate::ZcElem,
    core::{
        mem::{size_of, MaybeUninit},
        ptr, slice,
    },
    wincode::{
        config::ConfigCore,
        error::{ReadError, ReadResult, WriteResult},
        io::{Reader, Writer},
        SchemaRead, SchemaWrite, TypeMeta,
    },
};

macro_rules! storage_codec {
    ($ty:ty, $error:literal; $($generics:tt)*) => {
        // ZcElem guarantees an initialized, padding-free representation.
        unsafe impl<$($generics)* C: ConfigCore> SchemaWrite<C> for $ty {
            type Src = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: size_of::<Self>(),
                zero_copy: true,
            };

            fn size_of(_: &Self) -> WriteResult<usize> {
                Ok(size_of::<Self>())
            }

            fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
                let bytes = unsafe {
                    slice::from_raw_parts(src as *const Self as *const u8, size_of::<Self>())
                };
                writer.write(bytes)?;
                Ok(())
            }
        }

        // Every initialized bit pattern is Rust-valid; semantic validation must
        // run before publication, including when nested in another codec.
        unsafe impl<'de, $($generics)* C: ConfigCore> SchemaRead<'de, C> for $ty {
            type Dst = Self;
            const TYPE_META: TypeMeta = TypeMeta::Static {
                size: size_of::<Self>(),
                zero_copy: false,
            };

            fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
                let bytes = reader.take_scoped(size_of::<Self>())?;
                let value = unsafe { ptr::read_unaligned(bytes.as_ptr() as *const Self) };
                <Self as crate::ZcValidate>::validate_ref(&value)
                    .map_err(|_| ReadError::InvalidValue($error))?;
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
