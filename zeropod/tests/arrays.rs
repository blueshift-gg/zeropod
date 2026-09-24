//! `ArrayString` and `ArrayVec` keep their full capacity, so every field
//! of a struct of them stays where it is.

use zeropod::{ArrayString, ArrayVec, Error, Layout, ZeroPod};

#[derive(ZeroPod, Debug, PartialEq)]
struct Config {
    label: ArrayString<8>,
    members: ArrayVec<u32, 3>,
    threshold: u8,
}

#[test]
fn a_struct_of_arrays_has_one_size() {
    assert_eq!(<Config as ZeroPod>::SIZE, Some((4 + 8) + (4 + 3 * 4) + 1));
}

#[test]
fn writes_keep_every_field_in_place() {
    let config = Config {
        label: "vault".try_into().unwrap(),
        members: [1, 2].as_slice().try_into().unwrap(),
        threshold: 2,
    };
    let mut bytes = zeropod::to_vec(&config).unwrap();
    let view = Config::view_mut(&mut bytes).unwrap();

    view.set_label("treasury").unwrap();
    view.set_members(&[3]).unwrap();
    assert_eq!(view.set_label("too long!"), Err(Error::TooLong));

    assert_eq!((view.label(), view.threshold()), ("treasury", 2));
    assert_eq!(
        view.members()
            .iter()
            .map(|member| member.get())
            .collect::<Vec<_>>(),
        [3]
    );
    // The room past the items is zeroed, so equal values have equal bytes.
    let expected = Config {
        label: "treasury".try_into().unwrap(),
        members: [3].as_slice().try_into().unwrap(),
        threshold: 2,
    };
    assert_eq!(bytes, zeropod::to_vec(&expected).unwrap());
}

#[derive(ZeroPod, Debug)]
enum Label {
    Short(ArrayString<2>),
}

/// A `Ref` anyone can build: owning it checks the capacity, not trusts it.
#[test]
#[should_panic(expected = "fits its capacity")]
fn owning_a_string_past_its_capacity_panics_rather_than_overflows() {
    LabelRef::Short("too long").to_owned();
}
