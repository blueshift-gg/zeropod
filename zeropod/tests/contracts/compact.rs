use super::{corpus, fixed::Cell};
use std::hint::black_box;
use zeropod::{pod::*, ZcElem, ZcField, ZeroPod, ZeroPodCompact, ZeroPodError, ZeroPodFixed};

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
pub(super) struct Pair {
    first: PodVec<u8, 2, 1>,
    second: PodVec<u8, 2, 1>,
}

pub(super) fn check_pair(bytes: &mut [u8]) {
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

pub(super) fn check_compact(bytes: &mut [u8]) {
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
                check_record_edits(name, values, note);
            }
        }
    }
}

fn check_record_edits(name: &str, values: &[u16], note: Option<&str>) {
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
            assert_commit(result, &bytes, &before, &expected);
            if result.is_ok() {
                let read = RecordRef::new(&bytes).unwrap();
                assert_eq!(read.name(), next_name);
                assert_eq!(read.note(), next_note);
            }
        }
    }
}

#[derive(ZeroPod)]
#[zeropod(compact)]
struct Tails<T: ZcElem + ZcField<Pod = T>> {
    pub marker: T,
    first: PodVec<T, 2, 1>,
    maybe: Option<PodVec<T, 2, 4>>,
    last: PodVec<T, 2, 2>,
    text: Option<PodString<4, 8>>,
}

fn tail_bytes<T: AsRef<[u8]>>(
    marker: &T,
    first: &[T],
    maybe: Option<&[T]>,
    last: &[T],
    text: Option<&str>,
) -> Vec<u8> {
    let mut bytes = marker.as_ref().to_vec();
    bytes.extend([first.len() as u8, u8::from(maybe.is_some())]);
    bytes.extend((last.len() as u16).to_le_bytes());
    bytes.push(u8::from(text.is_some()));
    for value in first {
        bytes.extend(value.as_ref());
    }
    if let Some(maybe) = maybe {
        bytes.extend((maybe.len() as u32).to_le_bytes());
        for value in maybe {
            bytes.extend(value.as_ref());
        }
    }
    for value in last {
        bytes.extend(value.as_ref());
    }
    if let Some(text) = text {
        bytes.extend((text.len() as u64).to_le_bytes());
        bytes.extend(text.as_bytes());
    }
    bytes
}

#[test]
fn contract_mixed_tail_edits() {
    fn run<T: ZcElem + ZcField<Pod = T> + AsRef<[u8]> + Eq + std::fmt::Debug>(values: [T; 2]) {
        for first in [&values[..0], &values[..2]] {
            for maybe in [None, Some(&values[..0]), Some(&values[..2])] {
                for last in [&values[..0], &values[..2]] {
                    for text in [None, Some(""), Some("é")] {
                        check_tail_edits(&values, first, maybe, last, text);
                    }
                }
            }
        }
    }
    run([PodU16::from(513), PodU16::from(1027)]);
    run([[]; 2]);
}

fn check_tail_edits<T>(
    values: &[T; 2],
    first: &[T],
    maybe: Option<&[T]>,
    last: &[T],
    text: Option<&str>,
) where
    T: ZcElem + ZcField<Pod = T> + AsRef<[u8]> + Eq + std::fmt::Debug,
{
    let initial = tail_bytes(&values[0], first, maybe, last, text);
    for mask in 0..16 {
        let next_first = if mask & 1 != 0 { &values[..1] } else { first };
        let next_maybe = if mask & 2 == 0 {
            maybe
        } else {
            match maybe {
                None => Some(&values[..1]),
                Some([]) => Some(&values[..2]),
                Some(_) => None,
            }
        };
        let next_last = if mask & 4 != 0 {
            &values[..2 - last.len()]
        } else {
            last
        };
        let next_text = if mask & 8 == 0 {
            text
        } else {
            match text {
                None => Some("é"),
                Some("") => Some("xyz"),
                Some(_) => None,
            }
        };
        let expected = tail_bytes(&values[0], next_first, next_maybe, next_last, next_text);
        for capacity in [initial.len(), initial.len().max(expected.len()) + 1] {
            let mut storage = vec![0xa5; capacity + 2];
            storage[1..1 + initial.len()].copy_from_slice(&initial);
            let before = storage.clone();
            let bytes = &mut storage[1..1 + capacity];
            let mut view = TailsMut::<T>::new(bytes).unwrap();
            if mask & 1 != 0 {
                view.set_first(values).unwrap();
                view.set_first(next_first).unwrap();
            }
            if mask & 2 != 0 {
                view.set_maybe(next_maybe).unwrap();
            }
            if mask & 4 != 0 {
                view.set_last(next_last).unwrap();
            }
            if mask & 8 != 0 {
                view.set_text(next_text).unwrap();
            }
            assert_eq!(view.projected_size(), expected.len());
            let result = view.commit();
            if result.is_ok() {
                assert_eq!(view.commit(), result);
                let read = TailsRef::<T>::new(bytes).unwrap();
                assert_eq!(read.marker, values[0]);
                assert_eq!(read.first(), next_first);
                assert_eq!(read.maybe(), next_maybe);
                assert_eq!(read.last(), next_last);
                assert_eq!(read.text(), next_text);
            }
            assert_commit(result, bytes, &before[1..1 + capacity], &expected);
            assert_eq!(storage[0], 0xa5);
            assert_eq!(storage[capacity + 1], 0xa5);
        }
    }
}

#[test]
fn contract_commit_retry() {
    let mut bytes = [2, 0, 7, 8];
    let mut view = PairMut::new(&mut bytes).unwrap();
    view.set_second(&[9, 10]).unwrap();
    assert_eq!(view.set_second(&[1, 2, 3]), Err(ZeroPodError::Overflow));
    assert_eq!(view.commit(), Err(ZeroPodError::BufferTooSmall));
    view.set_first(&[]).unwrap();
    assert_eq!(view.commit(), Ok(4));
    assert_eq!(view.commit(), Ok(4));
    assert_eq!(bytes, [0, 2, 9, 10]);
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

pub(super) fn check_enum(bytes: &mut [u8]) {
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
                assert_commit(result, &bytes, &before, next);
            }
        }
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
        Err(ZeroPodError::Overflow)
    );
    assert_eq!(zero_sized.len(), 1);
}

fn assert_commit(
    result: Result<usize, ZeroPodError>,
    bytes: &[u8],
    before: &[u8],
    expected: &[u8],
) {
    if expected.len() <= bytes.len() {
        assert_eq!(result, Ok(expected.len()));
        assert_eq!(&bytes[..expected.len()], expected);
    } else {
        assert_eq!(result, Err(ZeroPodError::BufferTooSmall));
        assert_eq!(bytes, before);
    }
}
