#![allow(dead_code)]

use std::hint::black_box;
use zeropod::{pod::*, ZcElem, ZcField, ZeroPod, ZeroPodCompact, ZeroPodFixed};

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

#[derive(ZeroPod)]
struct Cell<T: ZcField> {
    pub value: T,
}

fn fixed_bytes<T: ZcField<Pod = T> + ZcElem>(seed: &[u8], read: impl Fn(&T)) {
    corpus(seed, |bytes| {
        let valid = Cell::<T>::validate(bytes).is_ok();
        assert_eq!(Cell::<T>::from_bytes(bytes).is_ok(), valid);
        if valid {
            read(&unsafe { Cell::<T>::from_bytes_unchecked(bytes) }.value);
            read(&unsafe { Cell::<T>::from_bytes_mut_unchecked(bytes) }.value);
        }
        if let Ok(view) = Cell::<T>::from_bytes_mut(bytes) {
            assert!(valid);
            read(&view.value);
            let mut copy = *view;
            std::mem::swap(view, &mut copy);
            read(&copy.value);
        } else {
            assert!(!valid);
        }
    });
}

fn read_text<const N: usize, const PFX: usize>(value: &PodString<N, PFX>) {
    assert!(value.decode_len() <= N);
    let expected = std::str::from_utf8(value.as_bytes()).unwrap();
    assert_eq!(value.chars().collect::<String>(), expected);
}

#[test]
fn contract_prefix_widths() {
    fn run<const PFX: usize>() {
        let mut text = vec![0; PFX];
        text[0] = 2;
        text.extend("é".as_bytes());
        fixed_bytes::<PodString<2, PFX>>(&text, read_text);
        let mut values = vec![0; PFX];
        values[0] = 1;
        values.extend([7, 0, 0, 0]);
        fixed_bytes::<PodVec<PodU16, 2, PFX>>(&values, |v| {
            assert!(v.decode_len() <= 2);
            for value in v.iter() {
                black_box(value.get());
            }
        });
    }
    run::<1>();
    run::<2>();
    run::<4>();
    run::<8>();
}

#[test]
fn contract_fixed_bytes() {
    fixed_bytes::<PodBool>(&[1], |v| {
        black_box(v.get());
    });
    fixed_bytes::<PodU128>(&u128::MAX.to_le_bytes(), |v| {
        black_box(v.get());
    });
    fixed_bytes::<PodI64>(&i64::MIN.to_le_bytes(), |v| {
        black_box(v.get());
    });
    fixed_bytes::<PodString<4>>(&[2, 0xc3, 0xa9, 0, 0], read_text);
    fixed_bytes::<PodVec<PodBool, 2>>(&[2, 0, 0, 1], |v| {
        for value in v.iter() {
            black_box(value.get());
        }
    });
    fixed_bytes::<PodOption<PodString<2>>>(&[1, 2, b'o', b'k'], |v| {
        if let Some(value) = v.get_ref() {
            read_text(value);
        }
    });
}

fn store<T: ZcField<Pod = T> + ZcElem>(value: T) {
    let mut bytes = vec![0; Cell::<T>::SIZE];
    Cell::<T>::from_bytes_mut(&mut bytes).unwrap().value = value;
    // Account storage must remain readable as bytes after safe typed assignment.
    for byte in &bytes {
        black_box(*byte);
    }
    assert!(Cell::<T>::validate(&bytes).is_ok());
}

#[test]
fn contract_fixed_string_writes() {
    for text in ["", "a", "é", "abcd"] {
        let mut value = PodString::<4>::default();
        value.try_set(text).unwrap();
        store(value);
    }
}

#[test]
fn contract_fixed_vec_writes() {
    for len in 0..=4 {
        let mut value = PodVec::<PodU16, 4>::default();
        value.try_set_from_slice(&[7u16.into(); 4][..len]).unwrap();
        for item in value.iter_mut() {
            item.set(8);
        }
        if let Some(last) = value.get_mut(len.saturating_sub(1)) {
            last.set(513);
        }
        assert!(value.get_mut(len).is_none());
        let mut expected = vec![8; len];
        if let Some(last) = expected.last_mut() {
            *last = 513;
        }
        assert_eq!(
            value.iter().map(|item| item.get()).collect::<Vec<_>>(),
            expected
        );
        store(value);
    }
}

#[test]
fn contract_string_sequences() {
    fn run<const N: usize, const PFX: usize>() {
        let mut pod = PodString::<N, PFX>::default();
        let mut model = String::new();
        for text in ["", "a", "é", "🦀", "abcde"] {
            for append in [false, true] {
                let expected = if append {
                    model.clone() + text
                } else {
                    text.to_owned()
                };
                let result = if append {
                    pod.try_push_str(text)
                } else {
                    pod.try_set(text)
                };
                assert_eq!(result.is_ok(), expected.len() <= N);
                if result.is_ok() {
                    model = expected;
                }
                assert_eq!(pod.as_str(), model);
                assert_eq!(pod.chars().collect::<String>(), model);
                for len in (0..=N + 1).rev() {
                    let mut truncated = pod;
                    let mut expected = model.clone();
                    let mut end = len.min(model.len());
                    while !model.is_char_boundary(end) {
                        end -= 1;
                    }
                    expected.truncate(end);
                    truncated.truncate(len);
                    assert_eq!(truncated.as_bytes(), expected.as_bytes());
                }
            }
        }
        pod.clear();
        assert!(pod.is_empty());
    }
    run::<0, 1>();
    run::<4, 1>();
    run::<4, 2>();
    run::<4, 4>();
    run::<4, 8>();
}

#[test]
fn contract_vec_sequences() {
    fn run<const N: usize, const PFX: usize>() {
        // Exhaust all four-operation sequences over this small operation alphabet.
        for sequence in 0..8usize.pow(4) {
            eprintln!("capacity={N} prefix={PFX} sequence={sequence}");
            let mut pod = PodVec::<u8, N, PFX>::default();
            let mut model = Vec::new();
            let mut operations = sequence;
            for _ in 0..4 {
                match operations % 8 {
                    0 => {
                        assert_eq!(pod.try_push(7).is_ok(), model.len() < N);
                        if model.len() < N {
                            model.push(7);
                        }
                    }
                    1 => assert_eq!(pod.pop(), model.pop()),
                    2 => {
                        assert_eq!(
                            pod.try_extend_from_slice(&[1, 2]).is_ok(),
                            model.len() + 2 <= N
                        );
                        if model.len() + 2 <= N {
                            model.extend([1, 2]);
                        }
                    }
                    3 => assert_eq!(
                        pod.remove(0),
                        if model.is_empty() {
                            None
                        } else {
                            Some(model.remove(0))
                        }
                    ),
                    4 => assert_eq!(
                        pod.swap_remove(0),
                        if model.is_empty() {
                            None
                        } else {
                            Some(model.swap_remove(0))
                        }
                    ),
                    5 => {
                        pod.retain(|v| v % 2 == 0);
                        model.retain(|v| v % 2 == 0);
                    }
                    6 => {
                        pod.truncate(1);
                        model.truncate(1);
                    }
                    _ => {
                        pod.clear();
                        model.clear();
                    }
                }
                assert_eq!(pod.as_slice(), model);
                assert_eq!(pod.get(N), None);
                operations /= 8;
            }
        }
    }
    run::<0, 1>();
    run::<2, 1>();
    run::<2, 2>();
    run::<2, 4>();
    run::<2, 8>();
}

#[test]
fn contract_option_sequences() {
    fn run<T: Copy + Eq + std::fmt::Debug, const PFX: usize>(values: &[T]) {
        let mut pod = PodOption::<T, PFX>::none();
        let mut model = None;
        for &value in values {
            assert_eq!(pod.get(), model);
            assert_eq!(pod.get_ref(), model.as_ref());
            assert_eq!(pod.replace(value), model.replace(value));
            assert_eq!(unsafe { pod.value_unchecked() }, &value);
            assert_eq!(pod.get_ref(), model.as_ref());
            assert_eq!(pod.take(), model.take());
            pod.set(Some(value));
            model = Some(value);
            assert_eq!(pod.get(), model);
            pod.clear();
            model = None;
            assert_eq!(pod.get(), model);
        }
    }
    run::<_, 1>(&[0u8, 1, 255]);
    run::<_, 2>(&[false, true]);
    run::<_, 4>(&[std::num::NonZeroU8::new(1).unwrap()]);
    run::<_, 1>(&["", "borrowed"]);
}

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Record {
    pub active: bool,
    pub name: zeropod::String<4>,
    pub values: zeropod::Vec<u16, 2>,
    pub note: Option<zeropod::String<4>>,
}

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Pair {
    first: PodVec<u8, 2, 1>,
    second: PodVec<u8, 2, 1>,
}

pub fn check_pair(bytes: &mut [u8]) {
    let valid = bytes.len() >= 2
        && bytes[0] <= 2
        && bytes[1] <= 2
        && 2 + bytes[0] as usize + bytes[1] as usize <= bytes.len();
    let view = PairRef::new(bytes);
    assert_eq!(view.is_ok(), valid);
    if let Ok(view) = view {
        let split = 2 + bytes[0] as usize;
        assert_eq!(view.first(), &bytes[2..split]);
        assert_eq!(view.second(), &bytes[split..split + bytes[1] as usize]);
    }
}

fn record_bytes(name: &str, values: &[u16], note: Option<&str>) -> Vec<u8> {
    let mut bytes = vec![
        1,
        name.len() as u8,
        values.len() as u8,
        0,
        u8::from(note.is_some()),
    ];
    bytes.extend(name.as_bytes());
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
    if let Some(note) = note {
        bytes.push(note.len() as u8);
        bytes.extend(note.as_bytes());
    }
    bytes
}

pub fn check_compact(bytes: &mut [u8]) {
    let valid = Record::validate(bytes).is_ok();
    assert_eq!(RecordRef::new(bytes).is_ok(), valid);
    if let Ok(view) = RecordRef::new(bytes) {
        black_box(view.active.get());
        black_box(view.name().chars().last());
        for value in view.values() {
            black_box(value.get());
        }
        if let Some(note) = view.note() {
            black_box(note.chars().last());
        }
    }
    assert_eq!(Record::header(bytes).is_ok(), valid);
    assert_eq!(RecordMut::new(bytes).is_ok(), valid);
    if valid {
        let before = bytes.to_vec();
        let len = RecordMut::new(bytes).unwrap().commit().unwrap();
        assert!(RecordRef::new(&bytes[..len]).is_ok());
        assert_eq!(bytes, before);
    }
}

#[test]
fn contract_compact_bytes() {
    corpus(&record_bytes("é", &[1, 513], Some("ok")), check_compact);
    corpus(&[2, 1, 7, 8, 9], check_pair);
}

#[test]
fn contract_compact_edits() {
    for name in ["", "é", "abcd"] {
        for values in [&[][..], &[1u16][..], &[2, 513][..]] {
            for note in [None, Some(""), Some("🦀")] {
                let initial = record_bytes(name, values, note);
                for mask in 0..8 {
                    let next_name = if mask & 1 != 0 { "xyz" } else { name };
                    let next_values = if mask & 2 != 0 { &[257][..] } else { values };
                    let next_note = if mask & 4 != 0 { Some("a") } else { note };
                    let expected = record_bytes(next_name, next_values, next_note);
                    for capacity in [initial.len(), initial.len().max(expected.len()) + 2] {
                        let mut bytes = initial.clone();
                        bytes.resize(capacity, 0xa5);
                        let before = bytes.clone();
                        let edits = [257u16.into()];
                        let mut view = RecordMut::new(&mut bytes).unwrap();
                        if mask & 1 != 0 {
                            view.set_name(next_name).unwrap();
                        }
                        if mask & 2 != 0 {
                            view.set_values(&edits).unwrap();
                        }
                        if mask & 4 != 0 {
                            view.set_note(next_note).unwrap();
                        }
                        assert_eq!(view.projected_size(), expected.len());
                        let result = view.commit();
                        if expected.len() <= capacity {
                            assert_eq!(result.unwrap(), expected.len());
                            assert_eq!(&bytes[..expected.len()], expected);
                            let read = RecordRef::new(&bytes).unwrap();
                            assert_eq!(read.name(), next_name);
                            assert_eq!(read.note(), next_note);
                        } else {
                            assert_eq!(result, Err(zeropod::ZeroPodError::BufferTooSmall));
                            assert_eq!(bytes, before);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn contract_abandoned_edits() {
    let mut bytes = record_bytes("a", &[7], Some("ok"));
    let before = bytes.clone();
    {
        let mut view = RecordMut::new(&mut bytes).unwrap();
        view.set_name("abcd").unwrap();
        assert!(view.set_note(Some("too long")).is_err());
    }
    assert_eq!(bytes, before);
}

#[test]
fn contract_header_transitions() {
    for initial in [
        record_bytes("", &[], None),
        record_bytes("abcd", &[7, 8], Some("ok")),
    ] {
        for replacement in [
            record_bytes("", &[], None),
            record_bytes("é", &[513], Some("a")),
        ] {
            for edit in [false, true] {
                let header = *Record::header(&replacement).unwrap();
                let mut bytes = initial.clone();
                let mut view = RecordMut::new(&mut bytes).unwrap();
                *view = header;
                if edit {
                    view.set_name("").unwrap();
                }
                let mut expected = initial.clone();
                expected[..Record::HEADER_SIZE]
                    .copy_from_slice(&replacement[..Record::HEADER_SIZE]);
                let validity = Record::validate(&expected);
                let result = view.commit();
                assert_eq!(result.is_ok(), validity.is_ok());
                if let Err(error) = validity {
                    assert_eq!(result, Err(error));
                }
            }
        }
    }
}

#[test]
fn contract_panicking_callback() {
    for panic_at in 0..4 {
        let mut values = PodVec::<u8, 4>::default();
        values.try_set_from_slice(&[1, 2, 3, 4]).unwrap();
        let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            values.retain(|value| {
                if calls == panic_at {
                    panic!("callback stopped");
                }
                calls += 1;
                value % 2 == 0
            });
        }));
        assert!(result.is_err());
        assert!(values.len() <= values.capacity());
        for value in values.as_slice() {
            black_box(*value);
        }
        values.clear();
        values.try_push(9).unwrap();
        assert_eq!(values.as_slice(), &[9]);
    }
}

mod messages {
    use zeropod::ZeroPod;

    #[derive(ZeroPod)]
    #[zeropod(compact)]
    #[repr(u16)]
    pub enum Message {
        Empty = 0,
        Text(zeropod::String<4>) = 1,
        Values(zeropod::Vec<u16, 2>) = 256,
        #[zeropod(compact)]
        Nested(super::Record) = 257,
    }
}
use messages::{Message, MessageMut, MessageRef};

pub fn check_enum(bytes: &mut [u8]) {
    match MessageRef::new(bytes) {
        Ok(value) => {
            assert!(Message::validate(bytes).is_ok());
            match value {
                MessageRef::Empty => (),
                MessageRef::Text(s) => {
                    black_box(s.chars().last());
                }
                MessageRef::Values(v) => {
                    for n in v {
                        black_box(n.get());
                    }
                }
                MessageRef::Nested(r) => {
                    black_box(r.name().chars().last());
                    for n in r.values() {
                        black_box(n.get());
                    }
                    if let Some(s) = r.note() {
                        black_box(s.chars().last());
                    }
                }
            }
            assert!(MessageMut::new(bytes).unwrap().commit().is_ok());
        }
        Err(error) => assert_eq!(Message::validate(bytes), Err(error)),
    }
}

#[test]
fn contract_enum_bytes() {
    for bytes in [
        vec![0, 0],
        vec![1, 0, 2, b'o', b'k'],
        vec![0, 1, 1, 0, 7, 0],
        [vec![1, 1], record_bytes("a", &[7], None)].concat(),
    ] {
        corpus(&bytes, check_enum);
    }
}

#[test]
fn contract_enum_edits() {
    let nested = record_bytes("é", &[7], None);
    let expected = [
        vec![0, 0],
        vec![1, 0, 2, b'o', b'k'],
        vec![0, 1, 1, 0, 7, 0],
        [vec![1, 1], nested.clone()].concat(),
    ];
    for initial in &expected {
        for (variant, next) in expected.iter().enumerate() {
            for capacity in [initial.len(), 32] {
                let mut bytes = initial.clone();
                bytes.resize(capacity, 0xa5);
                let before = bytes.clone();
                let values = [7u16.into()];
                let mut view = MessageMut::new(&mut bytes).unwrap();
                match variant {
                    0 => view.set_empty().unwrap(),
                    1 => view.set_text("ok").unwrap(),
                    2 => view.set_values(&values).unwrap(),
                    _ => view.set_nested(&nested).unwrap(),
                }
                let result = view.commit();
                if next.len() <= capacity {
                    assert_eq!(result.unwrap(), next.len());
                    assert_eq!(&bytes[..next.len()], next);
                } else {
                    assert_eq!(result, Err(zeropod::ZeroPodError::BufferTooSmall));
                    assert_eq!(bytes, before);
                }
            }
        }
    }
}

#[cfg(feature = "wincode")]
mod wire {
    use super::*;
    use wincode::{config::DefaultConfig, SchemaRead, SchemaWrite};

    fn decode<T>(seed: &[u8], read: impl Fn(&T))
    where
        T: for<'a> SchemaRead<'a, DefaultConfig, Dst = T> + SchemaWrite<DefaultConfig, Src = T>,
    {
        corpus(seed, |bytes| {
            if let Ok(value) = wincode::deserialize::<T>(bytes) {
                read(&value);
                let mut output = vec![0; seed.len() + 16];
                wincode::serialize_into(&mut output[..], &value).unwrap();
                let size = <T as SchemaWrite<DefaultConfig>>::size_of(&value).unwrap();
                assert_eq!(&output[..size], &bytes[..size]);
            }
        });
    }

    #[test]
    fn contract_wincode_bools() {
        for byte in 0..=u8::MAX {
            assert_eq!(wincode::deserialize::<PodBool>(&[byte]).is_ok(), byte <= 1);
            assert_eq!(
                wincode::deserialize::<[PodBool; 1]>(&[byte]).is_ok(),
                byte <= 1
            );
        }
        decode::<PodBool>(&[1], |value| {
            black_box(value.get());
        });
    }

    #[test]
    fn contract_wincode_strings() {
        decode::<PodString<2>>(&[2, b'o', b'k'], read_text);
        decode::<[PodString<2>; 1]>(&[2, b'o', b'k'], |v| {
            read_text(&v[0]);
        });
    }

    #[test]
    fn contract_wincode_vectors() {
        decode::<PodVec<PodString<2>, 1>>(&[1, 0, 2, b'o', b'k'], |v| {
            assert!(v.decode_len() <= 1);
            for s in v.iter() {
                read_text(s);
            }
        });
        decode::<[PodVec<PodString<2>, 1>; 1]>(&[1, 0, 2, b'o', b'k'], |v| {
            assert!(v[0].decode_len() <= 1);
            for s in v[0].iter() {
                read_text(s);
            }
        });
    }

    #[test]
    fn contract_wincode_options() {
        decode::<PodOption<PodString<2>>>(&[1, 2, b'o', b'k'], |v| {
            assert!(v.tag_valid());
            if let Some(s) = v.get_ref() {
                read_text(s);
            }
        });
        decode::<[PodOption<PodString<2>>; 1]>(&[1, 2, b'o', b'k'], |v| {
            assert!(v[0].tag_valid());
            if let Some(s) = v[0].get_ref() {
                read_text(s);
            }
        });
    }
}

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Wide {
    text: PodString<4, 8>,
    values: PodVec<PodU16, 2, 8>,
    optional: Option<PodVec<u8, 2, 8>>,
}

#[derive(ZeroPod)]
#[zeropod(compact)]
#[repr(u64)]
enum WideEnum {
    Empty = 0,
    Values(PodVec<PodU16, 2, 8>) = 1,
}

#[test]
fn contract_wide_lengths() {
    for length in [
        0,
        1,
        2,
        3,
        4,
        5,
        u32::MAX as u64,
        1 << 32,
        1 << 63,
        u64::MAX,
    ] {
        let mut fixed = length.to_le_bytes().to_vec();
        fixed.extend([b'a'; 4]);
        assert_eq!(
            Cell::<PodString<4, 8>>::validate(&fixed).is_ok(),
            length <= 4
        );
        assert_eq!(
            Cell::<PodVec<PodU16, 2, 8>>::validate(&fixed).is_ok(),
            length <= 2
        );
        for (offset, capacity) in [(0, 4), (8, 2), (17, 2)] {
            let mut bytes = vec![0; 29];
            bytes[16] = u8::from(offset == 17);
            bytes[offset..offset + 8].copy_from_slice(&length.to_le_bytes());
            assert_eq!(Wide::validate(&bytes).is_ok(), length <= capacity);
            if let Ok(view) = WideRef::new(&bytes) {
                black_box((view.text(), view.values(), view.optional()));
            }
        }
        let mut bytes = 1u64.to_le_bytes().to_vec();
        bytes.extend(length.to_le_bytes());
        bytes.extend([0; 4]);
        assert_eq!(WideEnum::validate(&bytes).is_ok(), length <= 2);
    }
}

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Large {
    values: PodVec<PodU16, { usize::MAX }, 8>,
}

#[test]
fn contract_length_overflow() {
    let bytes = (usize::MAX as u64).to_le_bytes();
    assert!(Large::validate(&bytes).is_err());
    let mut zero_sized = PodVec::<[u8; 0], 2>::default();
    zero_sized.try_push([]).unwrap();
    assert_eq!(
        zero_sized.try_extend_from_slice(&[[]; usize::MAX]),
        Err(zeropod::ZeroPodError::Overflow)
    );
    assert_eq!(zero_sized.len(), 1);
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[derive(ZeroPod)]
    #[zeropod(compact)]
    struct Values {
        values: PodVec<PodU16, 2, 1>,
    }

    #[kani::proof]
    #[kani::unwind(12)]
    fn compact_commit_preserves_unedited_tail() {
        let mut bytes: [u8; 6] = kani::any();
        let before = bytes;
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let Ok(view) = PairRef::new(&bytes[..len]) else {
            return;
        };
        let mut second = [0; 2];
        let count = bytes[1] as usize;
        assert_eq!(view.second().len(), count);
        second[..count].copy_from_slice(view.second());
        let replacement: [u8; 2] = kani::any();
        let new_len: usize = kani::any();
        kani::assume(new_len <= 2);
        let mut view = PairMut::new(&mut bytes[..len]).unwrap();
        view.set_first(&replacement[..new_len]).unwrap();
        let result = view.commit();
        if 2 + new_len + count <= len {
            assert_eq!(result.unwrap(), 2 + new_len + count);
            assert_eq!(bytes[0], new_len as u8);
            assert_eq!(&bytes[2..2 + new_len], &replacement[..new_len]);
            assert_eq!(&bytes[2 + new_len..2 + new_len + count], &second[..count]);
        } else {
            assert_eq!(result, Err(zeropod::ZeroPodError::BufferTooSmall));
            assert_eq!(bytes, before);
        }
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn compact_length_bounds() {
        let bytes: [u8; 5] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let view = ValuesRef::new(&bytes[..len]);
        assert_eq!(
            view.is_ok(),
            len >= 1 && bytes[0] <= 2 && 1 + 2 * bytes[0] as usize <= len
        );
        if let Ok(view) = view {
            for value in view.values() {
                black_box(value.get());
            }
        }
    }
}

pub fn check_storage(bytes: &mut [u8]) {
    check_compact(bytes);
    check_pair(bytes);
    check_enum(bytes);
    if let Ok(value) = Cell::<PodString<4>>::from_bytes(bytes) {
        read_text(&value.value);
    }
    if let Ok(value) = Cell::<PodVec<PodBool, 2>>::from_bytes(bytes) {
        for value in value.value.iter() {
            black_box(value.get());
        }
    }
    if let Ok(value) = Cell::<PodOption<PodString<4>>>::from_bytes(bytes) {
        if let Some(value) = value.value.get_ref() {
            read_text(value);
        }
    }
}
