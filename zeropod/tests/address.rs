//! Addresses read in place, alone and as a slice.
#![cfg(feature = "solana-address")]

use solana_address::Address;
use zeropod::{Layout, ZeroPod};

#[derive(ZeroPod)]
struct Multisig {
    creator: Address,
    #[max_len(3)]
    members: Vec<Address>,
}

#[test]
fn addresses_read_as_references_and_slices() {
    let (creator, members) = (
        Address::new_from_array([1; 32]),
        [2, 3].map(|b| Address::new_from_array([b; 32])),
    );
    let bytes = zeropod::to_vec(&Multisig {
        creator,
        members: members.to_vec(),
    })
    .unwrap();
    let view = Multisig::view(&bytes).unwrap();

    assert_eq!(view.creator(), &creator);
    assert!(view.members().contains(&members[1]));
    assert_eq!(&view.members()[..], &members);
}
