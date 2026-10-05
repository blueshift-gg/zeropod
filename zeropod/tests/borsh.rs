//! The bytes are Borsh's, both ways, for every kind of field.

mod common;

use common::{Profile, profile};
use proptest::prelude::*;
use zeropod::Layout;

proptest! {
    #[test]
    fn encodes_as_borsh_does(value in profile()) {
        let bytes = borsh::to_vec(&value).unwrap();
        prop_assert_eq!(zeropod::to_vec(&value).unwrap(), bytes.clone());
        prop_assert_eq!(zeropod::from_slice::<Profile>(&bytes).unwrap(), value.clone());
        prop_assert_eq!(Profile::size(Profile::view(&bytes).unwrap()), bytes.len());
    }
}
