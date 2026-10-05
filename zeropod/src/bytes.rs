//! The storage of every view.

/// Bytes holding a valid encoding. Its field is private to this crate, and
/// only this crate changes the bytes, to another valid encoding; so a view,
/// once made, stays valid, whatever the code around it does.
///
/// `Bytes` is guaranteed to be `#[repr(transparent)]` over `[u8]`: derived
/// views, themselves transparent over `Bytes`, are cast from `[u8]`.
#[repr(transparent)]
pub struct Bytes([u8]);

impl Bytes {
    /// The encoding, and any room after it.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// # Safety
    ///
    /// `bytes` start with a valid encoding of the type they are viewed as.
    #[doc(hidden)]
    pub unsafe fn new(bytes: &[u8]) -> &Self {
        // SAFETY: `Bytes` is a transparent `[u8]`.
        unsafe { &*(bytes as *const [u8] as *const Self) }
    }

    /// # Safety
    ///
    /// As for [`new`](Self::new).
    #[doc(hidden)]
    pub unsafe fn new_mut(bytes: &mut [u8]) -> &mut Self {
        // SAFETY: `Bytes` is a transparent `[u8]`.
        unsafe { &mut *(bytes as *mut [u8] as *mut Self) }
    }

    /// # Safety
    ///
    /// Every write leaves a valid encoding.
    pub(crate) unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
