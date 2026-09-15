use super::corpus;
use std::hint::black_box;
use zeropod::{pod::*, ZcElem, ZcField, ZeroPod, ZeroPodFixed};

#[derive(ZeroPod)]
pub(super) struct Cell<T: ZcField> {
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

pub(super) fn read_text<const N: usize, const PFX: usize>(value: &PodString<N, PFX>) {
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

pub(super) type NestedValue = PodOption<PodVec<PodOption<PodString<2, 4>>, 2, 1>, 2>;
pub(super) const NESTED_BYTES: &[u8] = &[
    1, 0, 1, 1, 2, 0, 0, 0, b'o', b'k', 255, 255, 255, 255, 255, 255, 255,
];

pub(super) fn read_nested(value: &NestedValue) {
    assert!(value.tag_valid());
    if let Some(values) = value.get_ref() {
        assert!(values.decode_len() <= 2);
        for value in values.iter() {
            assert!(value.tag_valid());
            if let Some(text) = value.get_ref() {
                read_text(text);
            }
        }
    }
}

#[test]
fn contract_nested_storage() {
    fixed_bytes::<NestedValue>(NESTED_BYTES, read_nested);
    let mut values = PodVec::default();
    values.try_push(PodOption::none()).unwrap();
    let mut text = PodString::default();
    text.try_set("é").unwrap();
    values.try_push(PodOption::some(text)).unwrap();
    let mut value = NestedValue::some(values);
    read_nested(&value);
    store(value);
    values = value.take().unwrap();
    assert!(values.remove(0).unwrap().is_none());
    value.set(Some(values));
    read_nested(&value);
    store(value);
    value.clear();
    store(value);
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
                assert_eq!(
                    pod.as_slice(),
                    model,
                    "capacity={N} prefix={PFX} sequence={sequence}"
                );
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

pub(super) fn check_storage(bytes: &mut [u8]) {
    if let Ok(value) = Cell::<NestedValue>::from_bytes(bytes) {
        read_nested(&value.value);
    }
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
