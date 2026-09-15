use super::compact::{PairMut, PairRef};
use std::hint::black_box;
use zeropod::{pod::*, ZeroPod};

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
