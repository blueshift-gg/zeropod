//! # Construction and lifetime contracts
//!
//! These consumers live in the same module as the derive. Generated field privacy
//! must therefore hold even when ordinary Rust field visibility permits access.
//!
//! A byte slice alone cannot construct a validated view.
//!
//! ```compile_fail,E0308
//! use zeropod::ZeroPod;
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { text: zeropod::String<4> }
//! let _ = TextRef { data: &[0] };
//! ```
//!
//! An existing view cannot be redirected to other storage through its state.
//!
//! ```compile_fail,E0594
//! use zeropod::ZeroPod;
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { text: zeropod::String<4> }
//! let mut original = [0];
//! let mut replacement = [1, 255];
//! let mut view = TextMut::new(&mut original).unwrap();
//! view.state.data = &mut replacement;
//! ```
//!
//! Validation for one schema does not authorize another schema's view.
//!
//! ```compile_fail,E0308
//! use zeropod::ZeroPod;
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Bytes { bytes: zeropod::Vec<u8, 4> }
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { text: zeropod::String<4> }
//! let bytes = BytesRef::new(&[0, 0]).unwrap();
//! let _ = TextRef { data: bytes.data };
//! ```
//!
//! Staged edits must outlive their commit.
//!
//! ```compile_fail,E0597
//! use zeropod::ZeroPod;
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { text: zeropod::String<4> }
//! let mut bytes = [0; 5];
//! let mut view = TextMut::new(&mut bytes).unwrap();
//! {
//!     let text = std::string::String::from("ok");
//!     view.set_text(&text).unwrap();
//! }
//! view.commit().unwrap();
//! ```
//!
//! A safe validation implementation cannot authorize raw typed construction.
//!
//! ```compile_fail,E0277
//! use zeropod::{ZeroPod, ZeroPodError, ZcField, ZcValidate};
//! #[derive(Clone, Copy)]
//! struct Field(bool);
//! impl ZcField for Field {
//!     type Pod = Self;
//!     const POD_SIZE: usize = 1;
//! }
//! impl ZcValidate for Field {
//!     fn validate_ref(_: &Self) -> Result<(), ZeroPodError> { Ok(()) }
//! }
//! #[derive(ZeroPod)]
//! struct Record { value: Field }
//! ```
//!
//! Prefixes must have an explicit supported width and sufficient capacity.
//!
//! ```compile_fail
//! use zeropod::ZeroPod;
//! const WIDTH: usize = 8;
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { value: zeropod::pod::PodString<4, WIDTH> }
//! ```
//!
//! ```compile_fail,E0080
//! use zeropod::{ZeroPod, ZeroPodCompact};
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! struct Text { value: zeropod::String<256> }
//! Text::validate(&[0]).unwrap();
//! ```
//!
//! A fixed payload's declared size must match its actual representation.
//!
//! ```compile_fail,E0080
//! use zeropod::{ZeroPod, ZeroPodFixed, ZeroPodSchema, ZeroPodError, LayoutKind};
//! struct Payload;
//! impl ZeroPodSchema for Payload { const LAYOUT: LayoutKind = LayoutKind::Fixed; }
//! impl ZeroPodFixed for Payload {
//!     type Zc = u8;
//!     const SIZE: usize = 4;
//!     fn validate(_: &[u8]) -> Result<(), ZeroPodError> { Ok(()) }
//!     fn from_bytes(data: &[u8]) -> Result<&u8, ZeroPodError> {
//!         data.first().ok_or(ZeroPodError::BufferTooSmall)
//!     }
//!     fn from_bytes_mut(data: &mut [u8]) -> Result<&mut u8, ZeroPodError> {
//!         data.first_mut().ok_or(ZeroPodError::BufferTooSmall)
//!     }
//! }
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! #[repr(u8)]
//! enum Message { Value(Payload) = 0 }
//! ```
//!
//! Compact enum elements need the same layout contract as fixed fields.
//!
//! ```compile_fail,E0277
//! use zeropod::{ZeroPod, ZeroPodError, ZcField, ZcValidate};
//! #[derive(Clone, Copy)]
//! struct Field(bool);
//! impl ZcField for Field { type Pod = Self; const POD_SIZE: usize = 1; }
//! impl ZcValidate for Field {
//!     fn validate_ref(_: &Self) -> Result<(), ZeroPodError> { Ok(()) }
//! }
//! struct Vec<T, const N: usize>([T; N]);
//! #[derive(ZeroPod)]
//! #[zeropod(compact)]
//! #[repr(u8)]
//! enum Message { Values(Vec<Field, 4>) = 0 }
//! ```

/// Borrowed decoding cannot bypass semantic validation.
///
/// ```compile_fail,E0277
/// use zeropod::pod::PodString;
/// let _: &PodString<2> = wincode::deserialize(&[1, b'a', 0]).unwrap();
/// ```
///
/// Raw serialization requires an initialized representation without padding.
///
/// ```compile_fail,E0277
/// use zeropod::pod::PodOption;
/// wincode::serialize_into(&mut [0; 16][..], &PodOption::<u64>::some(7)).unwrap();
/// ```
#[cfg(feature = "wincode")]
mod codecs {}
