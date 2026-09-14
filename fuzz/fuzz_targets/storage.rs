#![no_main]
#![allow(unexpected_cfgs)]
#[path = "../../zeropod/tests/contracts.rs"]
mod contracts;

libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    contracts::check_storage(&mut bytes.to_vec());
});
