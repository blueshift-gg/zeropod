//! One account-like value in each library's derive: fixed fields first, then
//! a string and a vector, as Solana programs store them.

use borsh::{BorshDeserialize, BorshSerialize};
use wincode::{SchemaRead, SchemaWrite};
use zeropod::ZeroPod;

#[derive(
    ZeroPod, BorshSerialize, BorshDeserialize, SchemaRead, SchemaWrite, Clone, Debug, PartialEq,
)]
pub struct Account {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
    #[max_len(32)]
    pub name: String,
    #[max_len(16)]
    pub members: Vec<[u8; 32]>,
}

/// The same fields as wincode borrows them, its fastest way to read.
#[derive(SchemaRead)]
pub struct AccountRef<'a> {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
    pub name: &'a str,
    pub members: &'a [[u8; 32]],
}

pub fn account() -> Account {
    Account {
        owner: [7; 32],
        amount: 1_000_000,
        bump: 254,
        name: "treasury".into(),
        members: (0..10).map(|member| [member; 32]).collect(),
    }
}
