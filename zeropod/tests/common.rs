//! One type of each kind the derive supports, in Borsh too.
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSerialize};
use proptest::prelude::*;
use zeropod::ZeroPod;

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub struct Stats {
    pub wins: u16,
    pub active: bool,
}

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Busy,
}

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum Role {
    Admin,
    Member {
        since: i64,
    },
    Custom(#[max_len(16)] String),
    /// A fixed field after a variable one: fine in a variant.
    Named {
        #[max_len(8)]
        name: String,
        rank: u8,
    },
}

#[derive(ZeroPod, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub level: u8,
    pub delta: i16,
    pub score: u64,
    pub huge: i128,
    pub key: [u8; 4],
    pub stats: Stats,
    pub status: Status,
    #[max_len(32)]
    pub name: String,
    #[max_len(8)]
    pub tags: Vec<u64>,
    #[max_len(4, 8)]
    pub aliases: Vec<String>,
    pub referrer: Option<u32>,
    #[max_len(8)]
    pub note: Option<String>,
    pub role: Role,
}

pub fn profile() -> impl Strategy<Value = Profile> {
    let fixed = (
        any::<(u8, i16, u64, i128, [u8; 4])>(),
        (any::<u16>(), any::<bool>(), any::<bool>()),
    );
    let variable = (
        "[a-z]{0,32}",
        prop::collection::vec(any::<u64>(), 0..=8),
        prop::collection::vec("[a-zé]{0,4}", 0..=4),
        any::<Option<u32>>(),
        prop::option::of("[a-z]{0,8}"),
        prop_oneof![
            Just(Role::Admin),
            any::<i64>().prop_map(|since| Role::Member { since }),
            "[a-z]{0,16}".prop_map(Role::Custom),
            ("[a-z]{0,8}", any::<u8>()).prop_map(|(name, rank)| Role::Named { name, rank }),
        ],
    );
    (fixed, variable).prop_map(
        |(
            ((level, delta, score, huge, key), (wins, active, busy)),
            (name, tags, aliases, referrer, note, role),
        )| {
            Profile {
                level,
                delta,
                score,
                huge,
                key,
                stats: Stats { wins, active },
                status: if busy { Status::Busy } else { Status::Idle },
                name,
                tags,
                aliases,
                referrer,
                note,
                role,
            }
        },
    )
}
