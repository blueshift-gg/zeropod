# zeropod

Zero-copy, alignment-1 pod types for Solana programs.

zeropod lets you read and write on-chain data through direct pointer casts — no serialization, no copies, no alignment traps. Every type is `#[repr(C)]` with alignment 1, so it maps directly onto Solana account bytes.

## Install

```toml
[dependencies]
zeropod = "0.1"
```

## Pod Types

All pod types are `Copy`, alignment 1, and safe to cast from arbitrary byte slices after validation.

| Type | Size | Description |
|------|------|-------------|
| `PodU16` .. `PodU128` | 2–16 | Unsigned integers, little-endian `[u8; N]` |
| `PodI16` .. `PodI128` | 2–16 | Signed integers, little-endian `[u8; N]` |
| `PodBool` | 1 | Boolean (byte must be 0 or 1) |
| `PodOption<T>` | 1 + size_of(T) | Optional value (tag byte + `MaybeUninit<T>`) |
| `PodString<N, PFX>` | PFX + N | UTF-8 string, length-prefixed, max N bytes |
| `PodVec<T, N, PFX>` | PFX + N * size_of(T) | Typed vector, length-prefixed, max N elements |

Convenience aliases: `zeropod::String<N>` = `PodString<N, 1>`, `zeropod::Vec<T, N>` = `PodVec<T, N, 2>`.

## Derive Macro

`#[derive(ZeroPod)]` generates a zero-copy companion type with validation and pointer-cast access.

### Fixed layout

Every field is a known size. The companion type is a direct `#[repr(C)]` mirror.

```rust
use zeropod::ZeroPod;

#[derive(ZeroPod)]
struct TokenAccount {
    pub mint: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u64,
    pub is_frozen: bool,
}

// Read from raw account bytes — validates, then pointer-casts (zero copy):
let zc = TokenAccount::from_bytes(&account_data)?;
let amount: u64 = zc.amount.get();
```

### Compact layout

For structs with variable-length fields. The on-chain format is `[fixed header + length prefixes][tail data]`. Fixed fields and length prefixes live in the header; dynamic data (strings, vecs) is packed contiguously after it.

```rust
use zeropod::ZeroPod;

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Profile {
    pub authority: [u8; 32],
    pub score: u64,
    pub name: zeropod::String<32>,
    pub tags: zeropod::Vec<u8, 16>,
}

// Read via zero-copy Ref:
let r = ProfileRef::new(&data)?;
let name: &str = r.name();
let tags: &[u8] = r.tags();

// Mutate via Mut + commit:
let mut m = ProfileMut::new(&mut data)?;
m.set_name("alice")?;
m.commit()?;
```

### Enums

Unit enums with `#[repr(u8)]` get a zero-copy companion that validates the discriminant.

```rust
#[derive(ZeroPod)]
#[repr(u8)]
enum Status {
    Inactive = 0,
    Active = 1,
    Frozen = 2,
}
```

## Arithmetic

Numeric pods use wrapping semantics in release builds and panic on overflow in debug builds — matching native integer behavior.

```rust
use zeropod::pod::PodU64;

let a = PodU64::from(100u64);
let b = PodU64::from(42u64);
assert_eq!((a + b).get(), 142);
assert_eq!((a - b).get(), 58);

// For security-sensitive code, use checked arithmetic:
assert_eq!(a.checked_sub(b), Some(PodU64::from(58)));
assert_eq!(b.checked_sub(a), None); // would underflow
```

## Validation

Every pod type implements `ZcValidate` — called automatically by `from_bytes()`. Validation rejects:

- `PodBool` with byte > 1
- `PodOption` with tag other than 0 or 1, or invalid inner value
- `PodString` with length > N or invalid UTF-8
- `PodVec` with length > N or invalid elements
- Enum discriminants outside the declared variants

```rust
// Malicious account data with bool byte = 5:
let mut buf = [0u8; 33];
buf[8] = 5; // invalid bool
assert!(TokenAccount::from_bytes(&buf).is_err());
```

## Traits

| Trait | Purpose |
|-------|---------|
| `ZeroPodSchema` | Declares fixed vs compact layout |
| `ZeroPodFixed` | Zero-copy access for fixed-size types |
| `ZeroPodCompact` | Zero-copy access for variable-length types |
| `ZcValidate` | Validates byte representations |
| `ZcElem` | Marker: alignment 1, valid for packed access (unsafe) |
| `ZcField` | Maps native Rust types to their pod companions |

## Feature Flags

| Flag | What it enables |
|------|----------------|
| `solana-address` | `ZcElem` + `ZcField` for `solana_address::Address` |
| `solana-program-error` | `From<ZeroPodError> for ProgramError` |
| `wincode` | `SchemaWrite` / `SchemaRead` for all pod types |

## Formal Verification

zeropod includes [Kani](https://model-checking.github.io/kani/) model-checking proofs covering:

- Roundtrip correctness for all pod types (encode -> decode preserves value)
- Length prefix encode/decode consistency across all prefix widths
- Bounds clamping (corrupted length prefixes cannot cause out-of-bounds access)
- Arithmetic operator consistency with native integers
- UTF-8 preservation in `PodString`
- `PodOption` tag semantics (invalid tags treated as None)
- Checked arithmetic matches `std` semantics

## License

Apache-2.0
