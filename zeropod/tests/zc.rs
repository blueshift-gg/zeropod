use zeropod::{Error, I16, I32, I64, I128, U16, U32, U64, U128, ZcElem, ZcValidate};

fn check_storage<T: ZcElem>(value: T) {
    assert_eq!(align_of::<T>(), 1);
    assert_eq!(T::validate_ref(&value), Ok(()));
}

#[test]
fn bytes_integers_and_arrays_are_fixed_storage() {
    check_storage(u8::MAX);
    check_storage(i8::MIN);
    check_storage(U16::new(u16::MAX));
    check_storage(U32::new(u32::MAX));
    check_storage(U64::new(u64::MAX));
    check_storage(U128::new(u128::MAX));
    check_storage(I16::new(i16::MIN));
    check_storage(I32::new(i32::MIN));
    check_storage(I64::new(i64::MIN));
    check_storage(I128::new(i128::MIN));
    check_storage([U64::new(42); 3]);
    check_storage([[U16::new(7); 2]; 3]);
    check_storage([0u8; 0]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Tagged {
    tag: u8,
    amount: U64,
}
impl ZcValidate for Tagged {
    fn validate_ref(value: &Self) -> Result<(), Error> {
        if value.tag <= 1 {
            Ok(())
        } else {
            Err(Error::InvalidTag)
        }
    }
}
// SAFETY: repr(C) of u8 and U64, both align-1, padding-free and valid for all bits.
unsafe impl ZcElem for Tagged {}

#[test]
fn custom_storage_and_arrays_keep_semantic_validation() {
    assert_eq!(size_of::<Tagged>(), 9);
    let valid = Tagged {
        tag: 1,
        amount: U64::new(42),
    };
    check_storage(valid);
    assert_eq!(<[Tagged; 2]>::validate_ref(&[valid; 2]), Ok(()));
    let invalid = Tagged { tag: 2, ..valid };
    assert_eq!(Tagged::validate_ref(&invalid), Err(Error::InvalidTag));
    assert_eq!(
        <[Tagged; 2]>::validate_ref(&[valid, invalid]),
        Err(Error::InvalidTag)
    );
    assert_eq!(
        <[[Tagged; 1]; 1]>::validate_ref(&[[invalid]]),
        Err(Error::InvalidTag)
    );
    assert_eq!(<[Tagged; 0]>::validate_ref(&[]), Ok(()));
}

#[cfg(feature = "solana-address")]
#[test]
fn addresses_are_fixed_storage() {
    let address = solana_address::Address::new_from_array([0xff; 32]);
    check_storage(address);
    check_storage([address; 2]);
}
