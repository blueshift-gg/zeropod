//! Traits for fixed, align-1 storage and its semantic validation.

use crate::{Error, I16, I32, I64, I128, U16, U32, U64, U128};

/// Checks semantic constraints on an already valid Rust value.
///
/// A byte can hold a tag or a length without every value making sense for the
/// application. Call this before interpreting such fields. Validation does not
/// establish the memory-safety requirements of [`ZcElem`].
pub trait ZcValidate {
    fn validate_ref(value: &Self) -> Result<(), Error>;
}

/// Fixed storage that can be borrowed directly from initialized bytes.
///
/// Unlike [`crate::Plain`], whose `Stored` may differ from `Self`, this trait
/// describes `Self`'s Rust layout. It does not implement a codec or make
/// `ZeroPod` views available; derive the required codec traits separately.
///
/// # Safety
///
/// Implementors must have alignment 1, no padding or uninitialized bytes, no
/// interior mutability, and no invalid Rust bit patterns. Any initialized byte
/// sequence of `size_of::<Self>()` bytes must be safe to interpret as `Self`,
/// including all zeroes. Semantic constraints may be checked by `ZcValidate`,
/// but forming a reference must be safe before validation.
///
/// Native multi-byte integers have alignment requirements:
/// ```compile_fail
/// fn storage<T: zeropod::ZcElem>() {}
/// storage::<u64>();
/// ```
/// `bool` has invalid bit patterns:
/// ```compile_fail
/// fn storage<T: zeropod::ZcElem>() {}
/// storage::<bool>();
/// ```
pub unsafe trait ZcElem: Copy + ZcValidate + 'static {}

macro_rules! plain {
    ($($ty:ty),* $(,)?) => {$(
        impl ZcValidate for $ty {
            #[inline(always)]
            fn validate_ref(_: &Self) -> Result<(), Error> { Ok(()) }
        }

        // SAFETY: an align-1 byte or byte wrapper; no padding, pointers,
        // interior mutability, or invalid bit patterns.
        unsafe impl ZcElem for $ty {}
    )*};
}

plain!(u8, i8, U16, U32, U64, U128, I16, I32, I64, I128);

impl<T: ZcValidate, const N: usize> ZcValidate for [T; N] {
    #[inline]
    fn validate_ref(value: &Self) -> Result<(), Error> {
        for item in value {
            T::validate_ref(item)?;
        }
        Ok(())
    }
}

// SAFETY: arrays add no padding, retain their elements' alignment, and have
// exactly the validity and initialization requirements of their elements.
unsafe impl<T: ZcElem, const N: usize> ZcElem for [T; N] {}

#[cfg(feature = "solana-address")]
plain!(solana_address::Address);
