//! Tuples: their items in order.

use crate::{
    __private::{from, from_unchecked, from_unchecked_mut, sum},
    Error, ZeroPod,
};

macro_rules! tuple {
    ($(($($item:ident $index:tt),+))*) => {$(
        // SAFETY: each method visits the items in order, through their own
        // implementations.
        unsafe impl<$($item: ZeroPod),+> ZeroPod for ($($item,)+) {
            type Ref<'a> = ($($item::Ref<'a>,)+) where Self: 'a;
            type In<'a> = ($($item::In<'a>,)+) where Self: 'a;
            const SIZE: Option<usize> = sum(&[$($item::SIZE),+]);
            const ANY_BYTES: bool = true $(&& $item::ANY_BYTES)+;

            fn check(bytes: &[u8], _: &[usize]) -> Result<usize, Error> {
                let mut at = 0;
                $(at += $item::check(from(bytes, at), &[])?;)+
                Ok(at)
            }

            unsafe fn len(bytes: &[u8]) -> usize {
                let mut at = 0;
                // SAFETY: the items follow one another.
                $(at += unsafe { $item::len(from_unchecked(bytes, at)) };)+
                at
            }

            #[allow(unused_assignments)]
            unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
                let mut at = 0;
                // SAFETY: as in `len`.
                unsafe {
                    ($({
                        let item = from_unchecked(bytes, at);
                        at += $item::len(item);
                        $item::read(item)
                    },)+)
                }
            }

            fn encoded_len(value: &Self::In<'_>, _: &[usize]) -> Result<usize, Error> {
                Ok(0 $(+ $item::encoded_len(&value.$index, &[])?)+)
            }

            unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize {
                let mut at = 0;
                // SAFETY: `out` holds every item, in order.
                $(at += unsafe { $item::write(&value.$index, from_unchecked_mut(out, at)) };)+
                at
            }

            fn max_len(_: &[usize]) -> Option<usize> {
                Some(0 $(+ $item::max_len(&[])?)+)
            }

            fn input(&self) -> Self::In<'_> {
                ($(self.$index.input(),)+)
            }

            fn own(value: Self::Ref<'_>) -> Self {
                ($($item::own(value.$index),)+)
            }
        }
    )*};
}

tuple! {
    (A 0)
    (A 0, B 1)
    (A 0, B 1, C 2)
    (A 0, B 1, C 2, D 3)
    (A 0, B 1, C 2, D 3, E 4)
    (A 0, B 1, C 2, D 3, E 4, F 5)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11)
}
