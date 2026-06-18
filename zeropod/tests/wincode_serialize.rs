//! Regression tests for wincode `SchemaWrite` on `PodString` / `PodVec`.
//!
//! These containers keep their data in a `[MaybeUninit<_>; N]` capacity array,
//! so only the length prefix and the first `len` bytes are initialized. The
//! serializer used to cast the whole struct to `&[u8]`, which read the
//! uninitialized capacity (UB under Miri) and leaked stale bytes into the
//! output, making serialization non-deterministic. Serialization must now emit
//! a fixed-size image that is fully initialized and depends only on the logical
//! value.
#![cfg(feature = "wincode")]

use zeropod::pod::{PodString, PodU16, PodVec};

#[test]
fn serialize_partial_podstring_zero_pads_capacity_and_roundtrips() {
    let mut s = PodString::<8>::default();
    assert!(s.set("hi"));

    let bytes = wincode::serialize(&s).unwrap();

    // PFX(1) + N(8) = 9 bytes: [len][active][zeroed capacity].
    assert_eq!(bytes.len(), core::mem::size_of::<PodString<8>>());
    assert_eq!(bytes, [2, b'h', b'i', 0, 0, 0, 0, 0, 0]);

    let back = wincode::deserialize::<PodString<8>>(&bytes).unwrap();
    assert_eq!(back.as_str(), "hi");
}

#[test]
fn serialize_partial_podvec_zero_pads_capacity_and_roundtrips() {
    let mut v = PodVec::<PodU16, 4, 2>::default();
    assert!(v.push(PodU16::from(5)));
    assert!(v.push(PodU16::from(7)));

    let bytes = wincode::serialize(&v).unwrap();

    // PFX(2) + N(4)*size_of(PodU16)(2) = 10 bytes.
    assert_eq!(bytes.len(), core::mem::size_of::<PodVec<PodU16, 4, 2>>());
    assert_eq!(bytes, [2, 0, 5, 0, 7, 0, 0, 0, 0, 0]);

    let back = wincode::deserialize::<PodVec<PodU16, 4, 2>>(&bytes).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back.as_slice(), &[PodU16::from(5), PodU16::from(7)]);
}

#[test]
fn serialize_is_deterministic_regardless_of_stale_capacity() {
    // `truncated` once held a longer value, so its capacity tail still contains
    // the old "defgh" bytes. `fresh` only ever held "abc". Both represent the
    // same logical string and must serialize to identical bytes.
    let mut truncated = PodString::<16>::default();
    assert!(truncated.set("abcdefgh"));
    truncated.truncate(3);
    assert_eq!(truncated.as_str(), "abc");

    let mut fresh = PodString::<16>::default();
    assert!(fresh.set("abc"));

    let a = wincode::serialize(&truncated).unwrap();
    let b = wincode::serialize(&fresh).unwrap();
    assert_eq!(
        a, b,
        "serialization must not depend on stale capacity bytes"
    );

    // And the stale "defgh" must not leak into the output.
    assert!(!a[4..].iter().any(|&byte| byte != 0));
}
