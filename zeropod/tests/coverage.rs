//! Every other kind of field Borsh stores, against borsh, both ways.
#![cfg(feature = "std")]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    marker::PhantomData,
};

use borsh::{BorshDeserialize, BorshSerialize};
use proptest::prelude::*;
use zeropod::{Layout, ZeroPod};

/// A newtype: stored and read as its field, and, being transparent, copied
/// whole in a vector as a `u64` is.
#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
struct Lamports(u64);

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct Point(i32, i32);

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct Pair<T> {
    first: T,
    rest: Vec<T>,
}

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
#[borsh(use_discriminant = true)]
enum Level {
    Low = 1,
    High = 5,
}

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct Everything {
    unit: (),
    marker: PhantomData<u8>,
    words: [u16; 3],
    flags: [bool; 2],
    boxed: Box<u32>,
    balance: Lamports,
    point: Point,
    level: Level,
    #[borsh(skip)]
    cache: u64,
    outcome: Result<u8, String>,
    tuple: (u8, String),
    names: BTreeMap<u16, String>,
    seen: BTreeSet<u8>,
    weights: HashMap<u8, u32>,
    tags: HashSet<u16>,
    pair: Pair<u16>,
    history: Vec<Lamports>,
}

fn everything() -> impl Strategy<Value = Everything> {
    let fixed = (
        any::<[u16; 3]>(),
        any::<[bool; 2]>(),
        any::<(u32, u64, i32, i32, bool)>(),
    );
    let variable = (
        any::<Result<u8, String>>(),
        any::<(u8, String)>(),
        any::<BTreeMap<u16, String>>(),
        any::<BTreeSet<u8>>(),
        any::<HashMap<u8, u32>>(),
        any::<HashSet<u16>>(),
        any::<(u16, Vec<u16>)>(),
        any::<Vec<u64>>(),
    );
    (fixed, variable).prop_map(|((words, flags, (boxed, balance, x, y, high)), variable)| {
        let (outcome, tuple, names, seen, weights, tags, (first, rest), history) = variable;
        Everything {
            unit: (),
            marker: PhantomData,
            words,
            flags,
            boxed: Box::new(boxed),
            balance: Lamports(balance),
            point: Point(x, y),
            level: if high { Level::High } else { Level::Low },
            cache: 0,
            outcome,
            tuple,
            names,
            seen,
            weights,
            tags,
            pair: Pair { first, rest },
            history: history.into_iter().map(Lamports).collect(),
        }
    })
}

proptest! {
    #[test]
    fn encodes_as_borsh_does(value in everything()) {
        let bytes = borsh::to_vec(&value).unwrap();
        prop_assert_eq!(zeropod::to_vec(&value).unwrap(), bytes.clone());
        prop_assert_eq!(zeropod::from_slice::<Everything>(&bytes).unwrap(), value);
    }
}

#[test]
fn a_skipped_field_is_not_stored_and_reads_as_default() {
    let value = Everything {
        cache: 99,
        ..everything_sample()
    };
    let bytes = zeropod::to_vec(&value).unwrap();
    assert_eq!(zeropod::from_slice::<Everything>(&bytes).unwrap().cache, 0);
}

#[test]
fn views_read_every_kind_in_place() {
    let bytes = zeropod::to_vec(&everything_sample()).unwrap();
    let view = Everything::view(&bytes).unwrap();

    assert_eq!(
        (view.balance(), view.point()._1(), view.boxed()),
        (7, -2, 3)
    );
    assert_eq!(view.words().map(|word| word.get()), [1, 2, 3]);
    assert!(matches!(view.level(), LevelRef::High));
    assert_eq!(view.outcome(), Err("no"));
    assert_eq!(view.tuple(), (4, "four"));
    assert_eq!(view.names().iter().collect::<Vec<_>>(), [(1, "one")]);
    assert_eq!(view.pair().rest().iter().collect::<Vec<_>>(), [8, 9]);
}

fn everything_sample() -> Everything {
    Everything {
        unit: (),
        marker: PhantomData,
        words: [1, 2, 3],
        flags: [true, false],
        boxed: Box::new(3),
        balance: Lamports(7),
        point: Point(-1, -2),
        level: Level::High,
        cache: 0,
        outcome: Err("no".into()),
        tuple: (4, "four".into()),
        names: BTreeMap::from([(1, "one".into())]),
        seen: BTreeSet::from([2]),
        weights: HashMap::from([(3, 30)]),
        tags: HashSet::from([5]),
        pair: Pair {
            first: 7,
            rest: vec![8, 9],
        },
        history: vec![Lamports(10)],
    }
}
