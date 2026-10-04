//! Mutable projections retain the complete enclosing encoding and its capacity.

use core::marker::PhantomData;

use crate::{__private::replace, Bytes, Error, ZeroPod, array::element_offset};

// Invoke an operation with a field's bounds. Keeping the provider rather than
// a borrowed temporary also supports max_len expressions evaluated at runtime.
type WithLimits = fn(&mut dyn FnMut(&[usize]) -> Result<(), Error>) -> Result<(), Error>;

/// An exclusive, typed part of a validated encoding. Writes preflight the value
/// and capacity before changing bytes, then move the entire enclosing suffix.
/// No mutable byte slice or standalone growing child view is exposed.
pub struct Edit<'a, T> {
    bytes: &'a mut Bytes,
    at: usize,
    total: fn(&[u8]) -> usize,
    with_limits: WithLimits,
    item: PhantomData<fn() -> T>,
}

impl<'a, T: ZeroPod> Edit<'a, T> {
    /// Validates a complete root encoding, with any trailing bytes as capacity.
    pub fn view(bytes: &'a mut [u8]) -> Result<Self, Error> {
        T::check(bytes, &[])?;
        // SAFETY: the root encoding was just validated.
        Ok(unsafe { Self::new(Bytes::new_mut(bytes)) })
    }

    /// Starts a validated root editor.
    ///
    /// # Safety
    /// `bytes` start with a valid T encoding, with no enclosing encoding.
    #[doc(hidden)]
    pub unsafe fn new(bytes: &'a mut Bytes) -> Self {
        Self {
            bytes,
            at: 0,
            // SAFETY: construction and every write preserve a valid root T.
            total: |bytes| unsafe { T::len(bytes) },
            with_limits: |with| with(&[]),
            item: PhantomData,
        }
    }

    pub fn read(&self) -> T::Ref<'_> {
        // SAFETY: this cursor always points at a valid T.
        unsafe { T::read(&self.bytes.as_slice()[self.at..]) }
    }

    /// Replaces this value. An error leaves every byte of the buffer unchanged.
    pub fn set(&mut self, value: T::In<'_>) -> Result<(), Error> {
        let total = (self.total)(self.bytes.as_slice());
        (self.with_limits)(&mut |limits| {
            // SAFETY: the projection retains the root and the field's bounds.
            unsafe { replace::<T>(self.bytes, self.at, || total, &value, limits) }
        })
    }

    /// Projects a field without losing the enclosing suffix or its capacity.
    ///
    /// # Safety
    /// `at` is a field offset within this T, with type U. `with_limits` must
    /// invoke its argument exactly once with that field's bounds and return its
    /// result. Replacing U within those bounds must preserve T and all
    /// enclosing types' validity (including bounds). No enclosing length or
    /// discriminant may need updating.
    #[doc(hidden)]
    pub unsafe fn project<U: ZeroPod>(self, at: usize, with_limits: WithLimits) -> Edit<'a, U> {
        Edit {
            bytes: self.bytes,
            at: self.at + at,
            total: self.total,
            with_limits,
            item: PhantomData,
        }
    }

    /// The current value's validated bytes, including the enclosing suffix.
    #[doc(hidden)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes.as_slice()[self.at..]
    }

    /// Borrows the cursor again so it can be used for more than one write.
    /// Sibling cursors cannot coexist across a write that might move bytes:
    ///
    /// ```compile_fail
    /// let mut bytes = [1u8, 2];
    /// let mut array = zeropod::Edit::<[u8; 2]>::view(&mut bytes).unwrap();
    /// let mut first = array.reborrow().get_mut(0).unwrap();
    /// let mut second = array.reborrow().get_mut(1).unwrap();
    /// first.set(3).unwrap();
    /// second.set(4).unwrap();
    /// ```
    pub fn reborrow(&mut self) -> Edit<'_, T> {
        Edit {
            bytes: self.bytes,
            at: self.at,
            total: self.total,
            with_limits: self.with_limits,
            item: PhantomData,
        }
    }

    /// Named field projections for a derived struct.
    pub fn fields(self) -> T::Fields<'a>
    where
        T: EditFields,
    {
        T::fields(self)
    }
}

impl<'a, T: ZeroPod, const N: usize> Edit<'a, [T; N]> {
    /// Borrows an element. Fixed-size indexing is constant time; otherwise the
    /// preceding elements are walked. Array bounds do not consume a max_len.
    pub fn get_mut(self, index: usize) -> Option<Edit<'a, T>> {
        if index >= N {
            return None;
        }
        // SAFETY: index is within the validated array.
        let at = unsafe { element_offset::<T>(self.as_bytes(), index) };
        let with_limits = self.with_limits;
        // SAFETY: arrays have no count or offsets to update, and pass the same
        // limits to every element, so replacement preserves the array.
        Some(unsafe { self.project(at, with_limits) })
    }
}

/// Named mutable projections generated alongside a struct's validated view.
pub trait EditFields: ZeroPod {
    type Fields<'a>
    where
        Self: 'a;

    fn fields<'a>(edit: Edit<'a, Self>) -> Self::Fields<'a>
    where
        Self: 'a;
}
