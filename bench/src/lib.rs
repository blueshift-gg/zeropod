//! One account-like value in each library's derive: fixed fields first, then
//! a string and a vector, as Solana programs store them. And its fixed
//! fields alone, for the libraries that cast bytes to a struct.

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

/// The fixed fields, which zeropod stores as Borsh does.
#[derive(ZeroPod, BorshSerialize)]
pub struct Fixed {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
}

/// The same bytes for bytemuck: packed, so no padding and alignment 1.
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
#[repr(C, packed)]
pub struct FixedMuck {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
}

/// The same bytes for zerocopy, with its unaligned little-endian integer.
#[derive(
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
    zerocopy::Unaligned,
)]
#[repr(C)]
pub struct FixedCopy {
    pub owner: [u8; 32],
    pub amount: zerocopy::little_endian::U64,
    pub bump: u8,
}

pub fn fixed() -> Fixed {
    Fixed {
        owner: [7; 32],
        amount: 1_000_000,
        bump: 254,
    }
}
