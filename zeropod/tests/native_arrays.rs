//! Native arrays have no count, including arrays of variable-length encodings.
use borsh::{BorshDeserialize, BorshSerialize};
use proptest::prelude::*;
use zeropod::{Edit, Error, Layout, ZeroPod};

#[derive(Clone, Copy, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "wincode", derive(wincode::SchemaRead, wincode::SchemaWrite))]
pub struct AssetConfig {
    pub balance: u64,
    pub limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "wincode", derive(wincode::SchemaRead, wincode::SchemaWrite))]
pub struct PositionInit {
    pub protocol: u8,
    pub max_staleness_seconds: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "wincode", derive(wincode::SchemaRead, wincode::SchemaWrite))]
pub struct Vault {
    pub assets: [AssetConfig; 8],
}

#[derive(Clone, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "wincode", derive(wincode::SchemaRead, wincode::SchemaWrite))]
pub struct InitializeArgs {
    pub positions: [PositionInit; 8],
}

#[derive(Clone, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
struct NamedPosition {
    protocol: u8,
    expiry: Option<u32>,
    #[max_len(8)]
    name: String,
}

#[derive(Clone, Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
struct Envelope {
    header: u64,
    positions: [[NamedPosition; 2]; 2],
    #[max_len(16)]
    tail: String,
}

fn envelope() -> Envelope {
    Envelope {
        header: 99,
        positions: core::array::from_fn(|i| {
            core::array::from_fn(|j| NamedPosition {
                protocol: (i * 2 + j) as u8,
                expiry: None,
                name: format!("{i}{j}"),
            })
        }),
        tail: "untouched".into(),
    }
}

fn parity<T>(value: &T) -> Vec<u8>
where
    T: ZeroPod + BorshSerialize + BorshDeserialize + PartialEq + core::fmt::Debug,
{
    let bytes = zeropod::to_vec(value).unwrap();
    assert_eq!(bytes, borsh::to_vec(value).unwrap());
    assert_eq!(&zeropod::from_slice::<T>(&bytes).unwrap(), value);
    assert_eq!(&borsh::from_slice::<T>(&bytes).unwrap(), value);
    bytes
}

#[test]
fn requested_types_and_fixed_derived_mutation() {
    let mut vault = Vault {
        assets: [AssetConfig {
            balance: 1,
            limit: 7,
        }; 8],
    };
    assert_eq!(Vault::SIZE, Some(128));
    let mut bytes = parity(&vault);
    let view = Vault::view_mut(&mut bytes).unwrap();
    assert_eq!(view.assets().get(7).unwrap().limit(), 7);
    assert_eq!(view.assets().iter().map(|a| a.balance()).sum::<u64>(), 8);
    assert!(view.assets().get(8).is_none());
    view.edit()
        .fields()
        .assets()
        .get_mut(3)
        .unwrap()
        .fields()
        .balance()
        .set(42)
        .unwrap();
    vault.assets[3].balance = 42;
    assert_eq!(bytes, parity(&vault));

    // Native padding and alignment never determine an element's wire size.
    #[derive(Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
    struct Padded {
        small: u8,
        large: u64,
    }
    let values = [Padded { small: 1, large: 2 }, Padded { small: 3, large: 4 }];
    assert_eq!(parity(&values).len(), 18);

    let args = InitializeArgs {
        positions: [PositionInit {
            protocol: 2,
            max_staleness_seconds: Some(30),
        }; 8],
    };
    assert_eq!(InitializeArgs::SIZE, None);
    let bytes = parity(&args);
    assert_eq!(bytes.len(), 8 * 6);
    assert_eq!(
        InitializeArgs::view(&bytes)
            .unwrap()
            .positions()
            .get(0)
            .unwrap()
            .max_staleness_seconds(),
        Some(30)
    );
    #[cfg(feature = "wincode")]
    {
        use wincode::config::{deserialize_exact, serialize};
        let config = zeropod::wincode::CONFIG;
        assert_eq!(serialize(&args, config).unwrap(), bytes);
        assert_eq!(
            deserialize_exact::<InitializeArgs, _>(&bytes, config).unwrap(),
            args
        );
        assert_eq!(serialize(&vault, config).unwrap(), parity(&vault));
    }
}

#[test]
fn nested_growth_and_shrink_preserve_every_later_element_and_field() {
    let mut expected = envelope();
    let mut bytes = parity(&expected);
    bytes.resize(Envelope::max_len(&[]).unwrap(), 0xa5);
    let view = Envelope::view_mut(&mut bytes).unwrap();
    // Growing a nested field moves its name, later elements, and the tail.
    view.edit()
        .fields()
        .positions()
        .get_mut(0)
        .unwrap()
        .get_mut(0)
        .unwrap()
        .fields()
        .expiry()
        .set(Some(123))
        .unwrap();
    expected.positions[0][0].expiry = Some(123);
    assert_eq!(view.to_owned(), expected);
    let mut positions = view.edit().fields().positions();
    positions
        .reborrow()
        .get_mut(0)
        .unwrap()
        .get_mut(1)
        .unwrap()
        .fields()
        .name()
        .set("longname")
        .unwrap();
    expected.positions[0][1].name = "longname".into();
    positions
        .reborrow()
        .get_mut(0)
        .unwrap()
        .get_mut(0)
        .unwrap()
        .fields()
        .expiry()
        .set(None)
        .unwrap();
    expected.positions[0][0].expiry = None;
    positions
        .get_mut(0)
        .unwrap()
        .get_mut(1)
        .unwrap()
        .fields()
        .name()
        .set("")
        .unwrap();
    expected.positions[0][1].name.clear();
    assert_eq!(view.to_owned(), expected);
    assert_eq!(&bytes[..parity(&expected).len()], parity(&expected));
}

#[test]
fn all_failed_writes_preserve_the_entire_buffer() {
    let expected = envelope();
    let mut bytes = parity(&expected); // no spare capacity
    let before = bytes.clone();
    assert_eq!(
        Envelope::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .positions()
            .get_mut(0)
            .unwrap()
            .get_mut(0)
            .unwrap()
            .fields()
            .expiry()
            .set(Some(1)),
        Err(Error::NoRoom)
    );
    assert_eq!(bytes, before);
    bytes.resize(bytes.len() + 32, 0xab);
    let before = bytes.clone();
    assert_eq!(
        Envelope::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .positions()
            .get_mut(0)
            .unwrap()
            .get_mut(0)
            .unwrap()
            .fields()
            .name()
            .set("123456789"),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
    let mut bad = expected.positions.clone();
    bad[1][1].name = "too long!x".into();
    // Even a failure in the last element must precede every write.
    assert_eq!(
        Envelope::view_mut(&mut bytes).unwrap().set_positions(&bad),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
    assert_eq!(zeropod::write(&bad, &mut bytes), Err(Error::TooLong));
    assert_eq!(bytes, before);
    assert!(
        Envelope::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .positions()
            .get_mut(2)
            .is_none()
    );
    assert_eq!(bytes, before);
}

#[test]
fn validates_each_element_and_propagates_array_bounds() {
    let bytes = parity(&envelope());
    for end in 0..bytes.len() {
        assert!(
            Envelope::view(&bytes[..end]).is_err(),
            "truncation at {end}"
        );
    }
    let mut bad = bytes.clone();
    bad[9] = 2; // first option tag, after header and protocol
    assert!(matches!(Envelope::view(&bad), Err(Error::InvalidTag)));
    bad = bytes.clone();
    bad[14] = 0xff; // first string's UTF-8
    assert!(matches!(Envelope::view(&bad), Err(Error::InvalidUtf8)));
    bad = bytes;
    bad[10..14].copy_from_slice(&9u32.to_le_bytes());
    assert!(matches!(Envelope::view(&bad), Err(Error::TooLong)));

    #[derive(ZeroPod)]
    struct Strings {
        #[max_len(3)]
        values: [[String; 2]; 1],
    }
    assert_eq!(Strings::max_len(&[]), Some(14));
    let mut bytes = zeropod::to_vec(&Strings {
        values: [["a".into(), "bbb".into()]],
    })
    .unwrap();
    bytes.resize(32, 0);
    let before = bytes.clone();
    assert_eq!(
        Strings::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .values()
            .get_mut(0)
            .unwrap()
            .get_mut(1)
            .unwrap()
            .set("four"),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
    let bad = borsh::to_vec(&[["a".to_string(), "four".to_string()]]).unwrap();
    assert!(matches!(Strings::view(&bad), Err(Error::TooLong)));
}

#[test]
fn empty_nested_and_zero_sized_arrays() {
    #[derive(Debug, PartialEq, ZeroPod, BorshSerialize, BorshDeserialize)]
    struct Empty {}
    assert_eq!(parity(&[Empty {}, Empty {}]), []);
    assert_eq!(parity(&[(); 4]), []);
    assert_eq!(parity(&[[(); 3]; 2]), []);
    assert_eq!(parity(&[] as &[String; 0]), []);
    assert_eq!(<[String; 0]>::SIZE, Some(0));
    assert_eq!(<[String; 0]>::max_len(&[]), Some(0));
    let empty = zeropod::read::<[String; 0]>(&[]).unwrap();
    assert!(empty.is_empty());
    assert!(empty.get(0).is_none());
    assert_eq!(empty.iter().len(), 0);
    let zst = zeropod::read::<[Empty; 2]>(&[]).unwrap();
    assert_eq!(zst.iter().count(), 2);
    assert!(zst.get(1).is_some());
    let mut bytes = [];
    Edit::<[(); 3]>::view(&mut bytes)
        .unwrap()
        .get_mut(2)
        .unwrap()
        .set(())
        .unwrap();
    assert!(
        Edit::<[(); 0]>::view(&mut bytes)
            .unwrap()
            .get_mut(0)
            .is_none()
    );
}

#[test]
fn primitive_array_stored_access_and_native_fast_paths() {
    let values = [0x1234u16, 0xabcd];
    let mut bytes = parity(&values);
    let view = zeropod::read::<[u16; 2]>(&bytes).unwrap();
    assert_eq!(view[0].get(), values[0]); // stored-array Deref
    assert_eq!(view.get(1), Some(values[1])); // natural-value generic API
    assert_eq!(view.iter().collect::<Vec<_>>(), values);
    assert_eq!(view.iter().nth(1), Some(values[1]));
    assert_eq!(view.iter().last(), Some(values[1]));
    let strings = parity(&["a".to_string(), "longer".to_string(), "z".to_string()]);
    let strings = zeropod::read::<[String; 3]>(&strings).unwrap();
    assert_eq!(strings.iter().nth(1), Some("longer"));
    assert_eq!(strings.iter().skip(1).collect::<Vec<_>>(), ["longer", "z"]);
    assert_eq!(strings.iter().last(), Some("z"));
    Edit::<[u16; 2]>::view(&mut bytes)
        .unwrap()
        .get_mut(1)
        .unwrap()
        .set(5)
        .unwrap();
    assert_eq!(bytes, parity(&[0x1234u16, 5]));
    parity(&[[1u64, 2], [3, 4]]);
    parity(&vec![[1u16, 2], [3, 4]]); // existing Vec<[T; N]> NATIVE path
    let flags = [true, false];
    assert_eq!(parity(&flags), [1, 0]);
    assert!(matches!(
        zeropod::read::<[bool; 2]>(&[1, 2]),
        Err(Error::InvalidBool)
    ));
}

proptest! {
    #[test]
    fn array_writes_match_borsh_and_are_atomic(
        index in 0usize..4,
        expiry in proptest::option::of(any::<u32>()),
        name in "[a-z]{0,10}",
        extra in 0usize..24,
    ) {
        let mut expected = envelope();
        let mut bytes = parity(&expected);
        bytes.resize(bytes.len() + extra, 0xa5);
        let before = bytes.clone();
        let replacement = NamedPosition { protocol: 7, expiry, name };
        let result = Envelope::view_mut(&mut bytes).unwrap().edit().fields().positions()
            .get_mut(index / 2).unwrap().get_mut(index % 2).unwrap().set(&replacement);
        let within_bounds = replacement.name.len() <= 8;
        expected.positions[index / 2][index % 2] = replacement;
        let encoded = borsh::to_vec(&expected).unwrap();
        let wanted = if !within_bounds { Err(Error::TooLong) }
            else if encoded.len() > bytes.len() { Err(Error::NoRoom) }
            else { Ok(()) };
        prop_assert_eq!(result, wanted);
        match result {
            Ok(()) => {
                prop_assert_eq!(parity(&expected), encoded.clone());
                prop_assert_eq!(&bytes[..encoded.len()], &encoded);
            }
            Err(_) => prop_assert_eq!(bytes, before),
        }
    }
}

#[test]
fn field_bound_expressions_need_not_be_promoted_to_static() {
    fn bound() -> usize {
        3
    }
    #[derive(ZeroPod)]
    struct Bounded {
        #[max_len(bound())]
        values: [String; 2],
    }
    let mut bytes = [0; 16];
    let view = Bounded::view_mut(&mut bytes).unwrap();
    view.edit()
        .fields()
        .values()
        .get_mut(1)
        .unwrap()
        .set("abc")
        .unwrap();
    let before = bytes;
    assert_eq!(
        Bounded::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .values()
            .get_mut(1)
            .unwrap()
            .set("long"),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
}
