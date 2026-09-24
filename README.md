# zeropod

Borsh's encoding, read and written in place.

Derive `ZeroPod` where you derive `BorshSerialize` and `BorshDeserialize`.
The bytes are the same, so anything that reads Borsh reads yours. On top of
that, zeropod reads and writes fields where they lie in the bytes, without
copying them into a struct first.

```rust
use zeropod::{Layout, ZeroPod};

#[derive(ZeroPod)]
pub struct Profile {
    pub owner: [u8; 32],
    pub score: u64,
    #[max_len(32)]
    pub name: String,
    #[max_len(10)]
    pub tags: Vec<u64>,
    pub role: Role,
}

#[derive(ZeroPod)]
pub enum Role {
    Admin,
    Member { since: i64 },
}

// As Borsh does: owned values in and out.
let bytes = zeropod::to_vec(&profile)?;
let profile: Profile = zeropod::from_slice(&bytes)?;

// In place: validated once, then every read is free.
let view = Profile::view(&bytes)?;
view.score();          // u64
view.name();           // &str
view.tags();           // &[U64]
view.role();           // RoleRef::Member { since }

// Written in place: later fields move within the room after the encoding.
let view = Profile::view_mut(&mut room)?;
view.set_name("ada")?;
let used = view.size();
```

## Types

| Field | Stored as |
|---|---|
| `u8`…`u128`, `i8`…`i128` | little-endian; read as the number, and in a vector as `U16`…`I128` |
| `bool` | one byte, 0 or 1 |
| `[u8; N]`, `Address` (feature `solana-address`) | the bytes |
| `String`, `Vec<T>` | a `u32` count, then the bytes or items |
| `Option<T>` | a 0 or 1 byte, then the value if 1 |
| a derived struct | its fields in order |
| a derived enum | a one-byte tag (the variant's index, or its written discriminant), then that variant's fields |
| `ArrayString<N>`, `ArrayVec<T, N>` | a `u32` count, then room for `N`: always the same size |

`#[max_len(N)]` bounds a string or a vector; `#[max_len(10, 32)]` bounds a
vector and the strings in it. It is checked when bytes are viewed and when a
field is written, and gives a type's largest encoding (`ZeroPod::max_len`).

Fields of fixed size come first, and fields whose size varies come last, so
every fixed field is at an offset known when compiling. `ArrayString` and
`ArrayVec` trade Borsh compatibility for a fixed size: a struct of fixed
fields only has one size, and writing it never moves anything.

## Safety

A view is a `Bytes`, whose bytes only zeropod can change, and only to
another valid encoding: once validated, a view stays valid, whatever the
code around it does. The `unsafe` lives in one implementation per wire type
and in the function that moves fields; the derive only calls them.

Checked in CI: a property test against the `borsh` crate for every kind of
field, both ways; property tests of writes and of corrupted bytes; Miri,
under stacked and tree borrows; and a Kani proof of the field move.

## Features

- `alloc` (default): `String`, `Vec`, `to_vec`.
- `solana-address`: `Address` fields.

`#[zeropod(crate = path)]` points the derive at a crate that re-exports
zeropod, for frameworks built on it.
