#![allow(dead_code)]

#[path = "contracts/compact.rs"]
mod compact;
#[path = "contracts/fixed.rs"]
mod fixed;
#[cfg(kani)]
#[path = "contracts/proofs.rs"]
mod proofs;
#[cfg(feature = "wincode")]
#[path = "contracts/wire.rs"]
mod wire;

// Every truncation, boundary-byte substitution, and a bounded multi-byte corpus.
fn corpus(seed: &[u8], mut check: impl FnMut(&mut [u8])) {
    let mut cases = vec![seed.to_vec()];
    for len in 0..seed.len() {
        cases.push(seed[..len].to_vec());
    }
    for index in 0..seed.len() {
        for byte in [0, 1, 2, 0x7f, 0x80, 0xff] {
            let mut bytes = seed.to_vec();
            bytes[index] = byte;
            cases.push(bytes);
        }
    }
    let mut state = 0x6a09e667f3bcc909u64;
    for len in 0..=seed.len() + 1 {
        cases.push(
            (0..len)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect(),
        );
    }
    for (case, bytes) in cases.iter().enumerate() {
        for offset in [0, 1, 7] {
            let mut storage = vec![0xa5; offset + bytes.len() + 1];
            storage[offset..offset + bytes.len()].copy_from_slice(bytes);
            eprintln!("case={case} offset={offset} bytes={bytes:02x?}");
            check(&mut storage[offset..offset + bytes.len()]);
            assert!(storage[..offset].iter().all(|&b| b == 0xa5));
            assert_eq!(storage[offset + bytes.len()], 0xa5);
        }
    }
}

pub fn check_storage(bytes: &mut [u8]) {
    fixed::check_storage(bytes);
    compact::check_compact(bytes);
    compact::check_pair(bytes);
    compact::check_enum(bytes);
}
