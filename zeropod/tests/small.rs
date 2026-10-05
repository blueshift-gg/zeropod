//! `SmallStr` and `SmallVec`: a string and a vector after a `u8` or a
//! `u16` count, which bounds them.

use proptest::prelude::*;
use zeropod::{Error, Layout, SmallStr, SmallVec, ZeroPod};

#[derive(ZeroPod, Clone, Debug, PartialEq)]
struct Entry {
    id: u8,
    name: SmallStr,
    scores: SmallVec<u64, u16>,
    #[max_len(2)]
    tags: SmallVec<SmallStr>,
}

fn entry(name: &str, scores: &[u64], tags: &[&str]) -> Entry {
    Entry {
        id: 7,
        name: name.into(),
        scores: scores.to_vec().into(),
        tags: tags
            .iter()
            .map(|tag| SmallStr::from(*tag))
            .collect::<Vec<_>>()
            .into(),
    }
}

/// The encoding, put together by hand: each count in its own width.
fn expected(entry: &Entry) -> Vec<u8> {
    let mut bytes = vec![entry.id, entry.name.len() as u8];
    bytes.extend(entry.name.as_bytes());
    bytes.extend((entry.scores.len() as u16).to_le_bytes());
    bytes.extend(entry.scores.iter().flat_map(|score| score.to_le_bytes()));
    bytes.push(entry.tags.len() as u8);
    for tag in entry.tags.iter() {
        bytes.push(tag.len() as u8);
        bytes.extend(tag.as_bytes());
    }
    bytes
}

proptest! {
    #[test]
    fn a_count_takes_its_width(
        name in "[a-zé]{0,40}",
        scores in prop::collection::vec(any::<u64>(), 0..=300),
        tags in prop::collection::vec("[a-z]{0,6}", 0..=2),
        cut in any::<prop::sample::Index>(),
    ) {
        let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
        let value = entry(&name, &scores, &tags);
        let bytes = zeropod::to_vec(&value).unwrap();
        prop_assert_eq!(&bytes, &expected(&value));
        prop_assert_eq!(zeropod::from_slice::<Entry>(&bytes).unwrap(), value);

        // Cut anywhere, the bytes are too short, not misread.
        prop_assert!(Entry::view(&bytes[..cut.index(bytes.len())]).is_err());
    }
}

#[test]
fn views_read_and_write_in_place() {
    let mut bytes = zeropod::to_vec(&entry("ab", &[1, 2], &["x"])).unwrap();
    bytes.resize(64, 0);
    let view = Entry::view_mut(&mut bytes).unwrap();
    assert_eq!((view.id(), view.name()), (7, "ab"));
    assert_eq!(view.scores().iter().sum::<u64>(), 3);
    assert_eq!(view.tags().iter().collect::<Vec<_>>(), ["x"]);

    view.set_name("lovelace").unwrap();
    view.set_scores(&[9]).unwrap();
    assert_eq!(view.to_owned(), entry("lovelace", &[9], &["x"]));
    assert_eq!(
        view.set_tags(&["a".into(), "b".into(), "c".into()]),
        Err(Error::TooLong)
    );
}

#[test]
fn the_count_bounds_the_length() {
    let name = "a".repeat(256);
    assert_eq!(
        zeropod::to_vec(&entry(&name, &[], &[])),
        Err(Error::TooLong)
    );
    assert!(zeropod::to_vec(&entry(&name[..255], &[], &[])).is_ok());
    assert_eq!(
        Entry::max_len(&[]),
        Some(1 + (1 + 255) + (2 + 65_535 * 8) + (1 + 2 * (1 + 255)))
    );
    assert_eq!(
        zeropod::read::<SmallStr>(&[1, 0xff]),
        Err(Error::InvalidUtf8)
    );
}

#[derive(ZeroPod)]
struct Loose(#[max_len(1000)] SmallStr);

/// A `#[max_len]` past the count's reach does not lift the count's bound.
#[test]
fn a_max_len_past_the_count_does_not_raise_it() {
    let name = "a".repeat(256);
    assert_eq!(
        zeropod::to_vec(&Loose(name.as_str().into())),
        Err(Error::TooLong)
    );
    assert_eq!(Loose::max_len(&[]), Some(1 + 255));
}
