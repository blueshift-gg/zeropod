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
| `u8`…`u128`, `i8`…`i128` | little-endian |
| `bool` | one byte, 0 or 1 |
| `()`, `PhantomData<T>` | nothing |
| `[T; N]` for any `T: ZeroPod`, `Address` (feature `solana-address`) | the items in place; arrays have no count |
| `String`, `Vec<T>` | a `u32` count, then the bytes or items |
| `BTreeMap`, `BTreeSet`, `HashMap`, `HashSet` (feature `std`) | a count, then the entries in key order |
| `Option<T>`, `Result<T, E>` | a tag, then the value |
| `Box<T>`, tuples | their contents |
| a derived struct, generic or not | its fields in order; one unnamed field is stored as that field |
| a derived enum | a one-byte tag (the variant's index, or its written discriminant), then that variant's fields |
| `ArrayString<N>`, `ArrayVec<T, N>` | a `u32` count, then room for `N`: always the same size |
| `SmallStr<u8>`, `SmallStr<u16>`, `SmallVec<T, u8>`, `SmallVec<T, u16>` | a `u8` or `u16` count, then the bytes or items |

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
`SmallStr` and `SmallVec` trade it for a shorter count: a `String` or a `Vec`
of at most 255 or 65,535 bytes or items, made with `.into()` and used as
the type it wraps.

## Arrays of native structs

Use native fields on chain and derive wincode's `SchemaRead`/`SchemaWrite`
in the off-chain client (with zeropod's `wincode` feature enabled):

```rust
use zeropod::{Layout, ZeroPod};
use wincode::{SchemaRead, SchemaWrite};

#[derive(Clone, Copy, ZeroPod, SchemaRead, SchemaWrite)]
pub struct AssetConfig {
    pub balance: u64,
    pub limit: u64,
}

#[derive(Clone, Copy, ZeroPod, SchemaRead, SchemaWrite)]
pub struct PositionInit {
    pub protocol: u8,
    pub max_staleness_seconds: Option<u32>,
}

#[derive(ZeroPod, SchemaRead, SchemaWrite)]
pub struct Vault {
    pub assets: [AssetConfig; 8], // fixed-size fields first
    pub positions: [PositionInit; 8],
}

let vault = Vault {
    assets: [AssetConfig { balance: 0, limit: 100 }; 8],
    positions: [PositionInit { protocol: 1, max_staleness_seconds: None }; 8],
};
let mut bytes = wincode::config::serialize(&vault, zeropod::wincode::CONFIG)?;
bytes.resize(Vault::max_len(&[]).unwrap(), 0);
let view = Vault::view_mut(&mut bytes)?;
assert_eq!(view.assets().get(0).unwrap().limit(), 100);
view.positions_mut().get_mut(0).unwrap()
    .fields().max_staleness_seconds().set(Some(30))?;
let used = view.size();
let decoded: Vault = wincode::config::deserialize_exact(
    &bytes[..used], zeropod::wincode::CONFIG,
)?;
assert_eq!(decoded.positions[0].max_staleness_seconds, Some(30));
```

Arrays encode exactly `N` consecutive elements, even for nested arrays,
empty arrays, or elements containing `Option` and bounded `String` fields.
Their borrowed `&Array<T, N>` view provides `len`, `is_empty`, `get`, and
`iter`, returning each element's usual borrowed form. Indexing fixed-size
elements is constant time; variable-size indexing walks preceding elements.
`#[max_len]` on an array passes its bounds to each element without consuming
a bound for the array itself.

`field_mut()` returns an exclusive `Edit<T>` cursor. Use `get_mut(index)` to
select an array element, `fields()` to select a derived struct's named field,
and `set(value)` to replace it. These projections consume the cursor;
`reborrow()` allows repeated edits through a retained cursor. `Edit::<T>::view`
starts an editor for a root value, including a standalone array. Every write
checks bounds and capacity before touching bytes, then moves all following
elements and enclosing fields. Failed writes leave the entire buffer
unchanged; a sequence of successful writes is not a transaction. No allocation
or mutable access to raw bytes is needed.

Compared with the initial 0.4 API, array setters now take `&[T; N]` (for
example, `view.set_assets(&assets)`), allowing non-`Copy` elements. Array
reads now return `&Array<T, N>` instead of `&[T::Stored; N]`. For `T: Plain`,
`Deref<Target = [T::Stored; N]>` preserves indexing and stored-array coercions;
`get` and `iter` now return `T::Ref` (for example, `u64` instead of `U64`).
Use `&**array_view` when an explicit stored array is needed. Primitive
validation and native-copy write fast paths remain. Derived structs are
never made `Plain`: their native layout may be padded, aligned, or variable.

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
- `wincode`: for a type shared with code that speaks wincode. Derive
  `SchemaWrite` and `SchemaRead` beside `ZeroPod`, and under
  `zeropod::wincode::CONFIG` wincode writes and reads zeropod's bytes,
  zeropod's own types included. `Result` and hash maps differ: wincode swaps
  the first's tags and does not sort the second.

`#[zeropod(crate = path)]` points the derive at a crate that re-exports
zeropod, for frameworks built on it.
