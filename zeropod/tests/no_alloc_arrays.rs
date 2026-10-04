//! Run with --no-default-features: arrays and their editors need no allocator.
use zeropod::{Error, Layout, ZeroPod};

#[derive(Clone, Copy, ZeroPod)]
struct Position {
    protocol: u8,
    expiry: Option<u32>,
}

#[derive(ZeroPod)]
struct Args<T, const N: usize> {
    positions: [T; N],
    tail: Option<u64>,
}

#[test]
fn arrays_and_nested_edits_without_alloc() {
    let mut bytes = [0xa5; 64];
    let args = Args {
        positions: [Position {
            protocol: 7,
            expiry: None,
        }; 2],
        tail: Some(99),
    };
    let len = zeropod::write(&args, &mut bytes).unwrap();
    assert_eq!(len, 13);
    let view = Args::<Position, 2>::view_mut(&mut bytes).unwrap();
    view.positions_mut()
        .get_mut(0)
        .unwrap()
        .fields()
        .expiry()
        .set(Some(42))
        .unwrap();
    assert_eq!(view.size(), 17);
    assert_eq!(view.positions().get(1).unwrap().protocol(), 7);
    assert_eq!(view.tail(), Some(99));
    let owned = zeropod::from_slice::<Args<Position, 2>>(&bytes[..17]).unwrap();
    assert_eq!(owned.positions[0].expiry, Some(42));
    let before = bytes;
    let view = Args::<Position, 2>::view_mut(&mut bytes[..17]).unwrap();
    assert_eq!(
        view.positions_mut()
            .get_mut(1)
            .unwrap()
            .fields()
            .expiry()
            .set(Some(1)),
        Err(Error::NoRoom)
    );
    assert_eq!(bytes, before);
}

#[derive(ZeroPod)]
struct Bounded<const N: usize> {
    #[max_len(N)]
    values: [zeropod::ArrayString<N>; 2],
}

#[test]
fn const_generic_field_bounds_and_empty_stored_arrays() {
    let mut bytes = [0; 16];
    let view = Bounded::<4>::view_mut(&mut bytes).unwrap();
    view.values_mut().get_mut(0).unwrap().set("four").unwrap();
    assert_eq!(view.values().get(0), Some("four"));
    let empty = zeropod::read::<[u8; 0]>(&[]).unwrap();
    let stored: &[u8; 0] = empty;
    assert_eq!(stored.len(), 0);
}
