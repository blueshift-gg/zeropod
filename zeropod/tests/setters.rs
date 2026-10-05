//! Writing through a view: the bytes end as the new value's encoding,
//! whatever the order of the writes.

mod common;

use common::{Profile, profile};
use proptest::prelude::*;
use zeropod::{Error, Layout};

/// Room for any `Profile` within its `#[max_len]`s.
const ROOM: usize = 1024;

proptest! {
    #[test]
    fn a_view_written_field_by_field_encodes_the_new_value(
        old in profile(),
        new in profile(),
        order in Just((0..13).collect::<Vec<usize>>()).prop_shuffle(),
    ) {
        let mut bytes = zeropod::to_vec(&old).unwrap();
        bytes.resize(ROOM, 0xAA);
        let view = Profile::view_mut(&mut bytes).unwrap();
        for field in order {
            match field {
                0 => view.set_level(new.level),
                1 => view.set_delta(new.delta),
                2 => view.set_score(new.score),
                3 => view.set_huge(new.huge),
                4 => view.set_key(new.key),
                5 => view.set_stats(&new.stats),
                6 => view.set_status(&new.status),
                7 => view.set_name(&new.name),
                8 => view.set_tags(&new.tags),
                9 => view.set_aliases(&new.aliases),
                10 => view.set_referrer(new.referrer),
                11 => view.set_note(new.note.as_deref()),
                _ => view.set_role(&new.role),
            }
            .unwrap();
        }
        let size = view.size();
        let expected = zeropod::to_vec(&new).unwrap();
        prop_assert_eq!(&bytes[..size], expected.as_slice());
    }
}

#[test]
fn a_write_past_the_room_fails_and_changes_nothing() {
    let value = Profile {
        name: "ada".into(),
        ..sample()
    };
    let mut bytes = zeropod::to_vec(&value).unwrap();
    let before = bytes.clone();
    let view = Profile::view_mut(&mut bytes).unwrap();

    assert_eq!(view.set_name("lovelace"), Err(Error::NoRoom));
    assert_eq!(bytes, before);
}

#[test]
fn a_write_past_its_max_len_fails_and_changes_nothing() {
    let mut bytes = zeropod::to_vec(&sample()).unwrap();
    bytes.resize(ROOM, 0);
    let before = bytes.clone();
    let view = Profile::view_mut(&mut bytes).unwrap();

    assert_eq!(view.set_name(&"x".repeat(33)), Err(Error::TooLong));
    assert_eq!(view.set_tags(&[0; 9]), Err(Error::TooLong));
    assert_eq!(bytes, before);
}

pub fn sample() -> Profile {
    Profile {
        level: 1,
        delta: -2,
        score: 3,
        huge: -4,
        key: [5; 4],
        stats: common::Stats {
            wins: 6,
            active: true,
        },
        status: common::Status::Busy,
        name: String::new(),
        tags: vec![7],
        aliases: vec!["é".into()],
        referrer: Some(8),
        note: None,
        role: common::Role::Member { since: 9 },
    }
}

#[test]
fn a_nested_value_is_changed_through_its_owned_copy() {
    let mut bytes = zeropod::to_vec(&sample()).unwrap();
    bytes.resize(ROOM, 0);
    let view = Profile::view_mut(&mut bytes).unwrap();

    let mut stats = view.stats().to_owned();
    stats.wins += 1;
    view.set_stats(&stats).unwrap();

    assert_eq!(view.stats().wins(), 7);
    assert_eq!(view.to_owned(), Profile { stats, ..sample() });
}
