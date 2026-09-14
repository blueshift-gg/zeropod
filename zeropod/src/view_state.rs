use core::{marker::PhantomData, ops::Deref};

/// Keeps generated view state private even in the module invoking the derive.
#[doc(hidden)]
#[repr(transparent)]
pub struct ViewState<S, T> {
    value: T,
    schema: PhantomData<fn(S) -> S>,
}

impl<S, T> ViewState<S, T> {
    /// # Safety
    /// The state must satisfy the invariants of the generated view for `S`,
    /// including validated storage bounds and live, valid edit pointers.
    #[inline(always)]
    pub unsafe fn new(value: T) -> Self {
        Self {
            value,
            schema: PhantomData,
        }
    }

    /// # Safety
    /// Changes must preserve the generated view's invariants before safe access resumes.
    #[inline(always)]
    pub unsafe fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<S, T> Deref for ViewState<S, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.value
    }
}
