//! Under zeropod's configuration, wincode writes and reads zeropod's bytes.
#![cfg(feature = "wincode")]

mod common;

use common::{Profile, profile};
use proptest::prelude::*;
use wincode::{
    SchemaRead, SchemaWrite,
    config::{deserialize_exact, deserialize_from, serialize},
    io::std_read::ReadAdapter,
};
use zeropod::{ArrayString, ArrayVec, SmallStr, SmallVec, ZeroPod, wincode::CONFIG};

/// zeropod's own types, which wincode knows through the `wincode` feature.
#[derive(ZeroPod, SchemaRead, SchemaWrite, Debug, PartialEq)]
struct Own {
    label: ArrayString<8>,
    members: ArrayVec<u32, 3>,
    flags: ArrayVec<bool, 2>,
    name: SmallStr,
    scores: SmallVec<u64, u16>,
    tags: SmallVec<SmallStr>,
}

proptest! {
    #[test]
    fn a_borsh_type_is_written_as_zeropod_writes_it(value in profile()) {
        let bytes = zeropod::to_vec(&value).unwrap();
        prop_assert_eq!(&serialize(&value, CONFIG).unwrap(), &bytes);
        prop_assert_eq!(deserialize_exact::<Profile, _>(&bytes, CONFIG).unwrap(), value);
    }

    #[test]
    fn zeropod_types_are_written_as_zeropod_writes_them(
        label in "[a-zé]{0,4}",
        members in prop::collection::vec(any::<u32>(), 0..=3),
        flags in prop::collection::vec(any::<bool>(), 0..=2),
        name in "[a-zé]{0,40}",
        scores in prop::collection::vec(any::<u64>(), 0..=300),
        tags in prop::collection::vec("[a-z]{0,6}", 0..=4),
    ) {
        let value = Own {
            label: label.as_str().try_into().unwrap(),
            members: members.as_slice().try_into().unwrap(),
            flags: flags.as_slice().try_into().unwrap(),
            name: name.into(),
            scores: scores.into(),
            tags: tags.into_iter().map(SmallStr::from).collect::<Vec<_>>().into(),
        };
        let bytes = zeropod::to_vec(&value).unwrap();
        prop_assert_eq!(&serialize(&value, CONFIG).unwrap(), &bytes);
        prop_assert_eq!(&deserialize_exact::<Own, _>(&bytes, CONFIG).unwrap(), &value);
        // A reader that lends no bytes: the array types copy theirs out.
        let reader = ReadAdapter::new(bytes.as_slice());
        prop_assert_eq!(deserialize_from::<Own, _>(reader, CONFIG).unwrap(), value);
    }
}

#[test]
fn what_zeropod_rejects_wincode_rejects() {
    let read = |bytes: &[u8]| deserialize_exact::<Own, _>(bytes, CONFIG);
    let value = Own {
        label: "ab".try_into().unwrap(),
        members: [1].as_slice().try_into().unwrap(),
        flags: [true].as_slice().try_into().unwrap(),
        name: "c".into(),
        scores: vec![2].into(),
        tags: vec![].into(),
    };
    let bytes = zeropod::to_vec(&value).unwrap();
    assert_eq!(read(&bytes).unwrap(), value);

    let with = |index: usize, byte: u8| {
        let mut bytes = bytes.clone();
        bytes[index] = byte;
        bytes
    };
    // `label`'s count past its capacity, and its text not UTF-8.
    assert!(read(&with(0, 9)).is_err());
    assert!(read(&with(4, 0xff)).is_err());
    // `flags`' first item not a `bool`.
    assert!(read(&with(12 + 16 + 4, 2)).is_err());
    // `name` not UTF-8.
    assert!(read(&with(12 + 16 + 6 + 1, 0xff)).is_err());
    assert!(read(&bytes[..bytes.len() - 1]).is_err());
}
