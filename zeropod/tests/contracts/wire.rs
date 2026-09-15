use super::{
    corpus,
    fixed::{read_nested, read_text, NestedValue, NESTED_BYTES},
};
use std::hint::black_box;
use wincode::{config::DefaultConfig, SchemaRead, SchemaWrite};
use zeropod::pod::*;

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
    decode::<NestedValue>(NESTED_BYTES, read_nested);
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
