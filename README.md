# zeropod

Zero-copy, alignment-1 pod types for Solana programs.

## Overview

zeropod provides `#[repr(C)]`, alignment-1 types that can be safely cast from raw byte slices — the foundation for zero-copy account access on Solana's BPF runtime.

**Pod types:** `PodU16`, `PodU32`, `PodU64`, `PodU128`, `PodI16`, `PodI32`, `PodI64`, `PodI128`, `PodBool`, `PodOption<T>`, `PodString<N>`, `PodVec<T, N>`

**Derive macro:** `#[derive(ZeroPod)]` generates zero-copy companions with validation, pointer-cast access, and compact (variable-length) layout support.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
zeropod = "0.1"
```

### Fixed layout (all fields are fixed-size)

```rust
use zeropod::ZeroPod;

#[derive(ZeroPod)]
struct TokenAccount {
    pub mint: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u64,
    pub is_frozen: bool,
}

// Zero-copy read from raw bytes:
let zc = TokenAccount::from_bytes(&data)?;
assert_eq!(zc.amount.get(), 1_000_000);
```

### Compact layout (variable-length tail fields)

```rust
use zeropod::ZeroPod;

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Profile {
    pub authority: [u8; 32],
    pub score: u64,
    pub name: zeropod::String<32>,       // variable-length string (max 32 bytes)
    pub tags: zeropod::Vec<u8, 16>,      // variable-length vec (max 16 elements)
}

// Compact layout: [fixed header + length prefixes][tail data]
// Read via Ref, mutate via Mut + commit().
```

### Arithmetic

Pod numeric types use wrapping semantics in release builds and panic on overflow in debug builds — matching native integer behavior. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` for explicit overflow detection.

```rust
use zeropod::pod::PodU64;

let a = PodU64::from(100u64);
let b = PodU64::from(42u64);
let c = a + b;
assert_eq!(c.get(), 142);

// Checked arithmetic for security-sensitive code:
let result = a.checked_sub(b); // Some(PodU64(58))
```

## Features

- **`solana-address`** — `ZcElem` and `ZcField` impls for `solana_address::Address`
- **`solana-program-error`** — `From<ZeroPodError> for ProgramError`
- **`wincode`** — `SchemaWrite`/`SchemaRead` impls for all pod types

## Safety

All `unsafe` code has SAFETY comments. The crate includes [Kani](https://model-checking.github.io/kani/) model-checking proofs for critical invariants (roundtrip correctness, bounds, UTF-8 preservation).

## License

Apache-2.0
