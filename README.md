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
| `u8`…`u128`, `i8`…`i128`, `f32`, `f64` | little-endian; a NaN is refused, as Borsh refuses it |
| `bool` | one byte, 0 or 1 |
| `()`, `PhantomData<T>` | nothing |
| `[T; N]` of fixed-size items, `Address` (feature `solana-address`) | the items in place |
| `String`, `Vec<T>` | a `u32` count, then the bytes or items |
| `BTreeMap`, `BTreeSet`, `HashMap`, `HashSet` (feature `std`) | a count, then the entries in key order |
| `Option<T>`, `Result<T, E>` | a tag, then the value |
| `Box<T>`, tuples | their contents |
| a derived struct, generic or not | its fields in order; one unnamed field is stored as that field |
| a derived enum | a one-byte tag (the variant's index, or its written discriminant), then that variant's fields |
| `ArrayString<N>`, `ArrayVec<T, N>` | a `u32` count, then room for `N`: always the same size |

Reading in place gives each field's natural form: numbers by value; `&str`,
`&[U64]` and other slices; `&Items<T>` to iterate strings, tuples or map
entries; a view of a nested struct; an enum's `Ref`.

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

## Your own types

Derive `ZeroPod`; `#[borsh(skip)]` and `#[borsh(use_discriminant = ..)]`
mean what they mean to Borsh, so a Borsh type derives unchanged. A newtype,
`struct Lamports(u64)`, is stored as its field. Anything else implements the
`unsafe trait ZeroPod` by hand, under the contract its documentation spells
out.

## Features

- `alloc` (default): `String`, `Vec`, `Box`, B-tree maps and sets, `to_vec`.
- `std`: hash maps and sets.
- `solana-address`: `Address` fields.
- `solana-program-error`: `?` from `zeropod::Error` into `ProgramError`.

`#[zeropod(crate = path)]` points the derive at a crate that re-exports
zeropod, for frameworks built on it.
