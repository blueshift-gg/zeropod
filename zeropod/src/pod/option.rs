use core::mem::MaybeUninit;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PodOption<T: Copy> {
    tag: u8,
    value: MaybeUninit<T>,
}

const _: () = assert!(core::mem::align_of::<PodOption<u8>>() == 1);

impl<T: Copy> PodOption<T> {
    #[inline(always)]
    pub fn none() -> Self {
        Self {
            tag: 0,
            value: MaybeUninit::zeroed(),
        }
    }
    #[inline(always)]
    pub fn some(value: T) -> Self {
        Self {
            tag: 1,
            value: MaybeUninit::new(value),
        }
    }
    #[inline(always)]
    pub fn is_some(&self) -> bool {
        self.tag == 1
    }
    #[inline(always)]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }
    #[inline(always)]
    pub fn get(&self) -> Option<T> {
        if self.is_some() {
            Some(unsafe { self.value.assume_init() })
        } else {
            None
        }
    }
    #[inline(always)]
    pub fn set(&mut self, value: Option<T>) {
        match value {
            Some(v) => {
                self.tag = 1;
                self.value = MaybeUninit::new(v);
            }
            None => {
                self.tag = 0;
                self.value = MaybeUninit::zeroed();
            }
        }
    }
    pub fn raw_tag(&self) -> u8 {
        self.tag
    }

    /// # Safety
    /// Caller must ensure tag == 1 (Some).
    #[inline(always)]
    pub unsafe fn assume_init_ref(&self) -> &T {
        self.value.assume_init_ref()
    }

    pub fn take(&mut self) -> Option<T> {
        let result = self.get();
        self.tag = 0;
        self.value = MaybeUninit::zeroed();
        result
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        let old = self.get();
        self.tag = 1;
        self.value = MaybeUninit::new(value);
        old
    }

    pub fn clear(&mut self) {
        self.tag = 0;
        self.value = MaybeUninit::zeroed();
    }

    pub fn unwrap_or(self, default: T) -> T {
        match self.get() {
            Some(v) => v,
            None => default,
        }
    }

    pub fn map_or<U>(&self, default: U, f: impl FnOnce(T) -> U) -> U {
        match self.get() {
            Some(v) => f(v),
            None => default,
        }
    }
}

impl<T: Copy> Default for PodOption<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: Copy + PartialEq> PartialEq for PodOption<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.get(), other.get()) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + Eq> Eq for PodOption<T> {}

impl<T: Copy + PartialEq> PartialEq<Option<T>> for PodOption<T> {
    fn eq(&self, other: &Option<T>) -> bool {
        match (self.get(), other) {
            (Some(a), Some(b)) => a == *b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + core::fmt::Debug> core::fmt::Debug for PodOption<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get() {
            Some(v) => write!(f, "Some({:?})", v),
            None => write!(f, "None"),
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn some_roundtrip() {
        let v: u8 = kani::any();
        let pod = PodOption::some(v);
        assert!(pod.is_some());
        assert!(!pod.is_none());
        assert!(pod.get() == Some(v), "some roundtrip must preserve value");
    }

    #[kani::proof]
    fn none_roundtrip() {
        let pod = PodOption::<u8>::none();
        assert!(pod.is_none());
        assert!(!pod.is_some());
        assert!(pod.get() == None, "none must return None");
    }

    #[kani::proof]
    fn set_some_then_get() {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8>::none();
        pod.set(Some(v));
        assert!(pod.get() == Some(v), "set(Some(v)) then get() must return Some(v)");
    }

    #[kani::proof]
    fn set_none_then_get() {
        let v: u8 = kani::any();
        let mut pod = PodOption::some(v);
        pod.set(None);
        assert!(pod.get() == None, "set(None) then get() must return None");
    }

    #[kani::proof]
    fn take_returns_value_and_clears() {
        let v: u8 = kani::any();
        let mut pod = PodOption::some(v);
        let taken = pod.take();
        assert!(taken == Some(v), "take must return the value");
        assert!(pod.is_none(), "take must clear to None");
    }

    #[kani::proof]
    fn replace_returns_old() {
        let old: u8 = kani::any();
        let new: u8 = kani::any();
        let mut pod = PodOption::some(old);
        let returned = pod.replace(new);
        assert!(returned == Some(old), "replace must return old value");
        assert!(pod.get() == Some(new), "replace must set new value");
    }

    #[kani::proof]
    fn replace_on_none_returns_none() {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8>::none();
        let returned = pod.replace(v);
        assert!(returned == None, "replace on None must return None");
        assert!(pod.get() == Some(v), "replace must set value");
    }

    #[kani::proof]
    fn invalid_tag_is_none() {
        let tag: u8 = kani::any();
        kani::assume(tag != 0 && tag != 1);
        let mut buf = [0u8; 2]; // PodOption<u8>: tag + value
        buf[0] = tag;
        buf[1] = kani::any();
        let pod = unsafe { &*(buf.as_ptr() as *const PodOption<u8>) };
        // Invalid tags must NOT be treated as Some.
        assert!(!pod.is_some(), "invalid tag must not be Some");
        assert!(pod.get() == None, "invalid tag must return None from get()");
    }

    #[kani::proof]
    fn unwrap_or_some() {
        let v: u8 = kani::any();
        let default: u8 = kani::any();
        let pod = PodOption::some(v);
        assert!(pod.unwrap_or(default) == v, "unwrap_or on Some must return value");
    }

    #[kani::proof]
    fn unwrap_or_none() {
        let default: u8 = kani::any();
        let pod = PodOption::<u8>::none();
        assert!(pod.unwrap_or(default) == default, "unwrap_or on None must return default");
    }

    #[kani::proof]
    fn default_is_none() {
        let pod = PodOption::<u8>::default();
        assert!(pod.is_none(), "default must be None");
        assert!(pod.raw_tag() == 0, "default tag must be 0");
    }

    #[kani::proof]
    fn clear_makes_none() {
        let v: u8 = kani::any();
        let mut pod = PodOption::some(v);
        pod.clear();
        assert!(pod.is_none(), "clear must make None");
    }
}
