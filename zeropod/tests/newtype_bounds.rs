use zeropod::{Error, Layout, ZeroPod};

#[derive(Debug, PartialEq, ZeroPod)]
struct Name(String);

#[derive(Debug, PartialEq, ZeroPod)]
struct Holder {
    #[max_len(4)]
    name: Name,
    tail: Option<u8>,
}

#[test]
fn a_newtype_inherits_its_containing_fields_bounds() {
    let long = "twelve bytes";
    let bad = Holder {
        name: Name(long.into()),
        tail: Some(7),
    };
    assert_eq!(zeropod::to_vec(&bad), Err(Error::TooLong));
    assert_eq!(Holder::max_len(&[]), Some(10));
    let invalid = borsh::to_vec(&(long, Some(7u8))).unwrap();
    assert!(matches!(Holder::view(&invalid), Err(Error::TooLong)));
    let good = Holder {
        name: Name("four".into()),
        tail: Some(7),
    };
    let mut bytes = zeropod::to_vec(&good).unwrap();
    assert_eq!(zeropod::from_slice::<Holder>(&bytes).unwrap(), good);
    bytes.resize(32, 0xa5);
    let before = bytes.clone();
    assert_eq!(
        Holder::view_mut(&mut bytes).unwrap().set_name(long),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
    assert_eq!(
        Holder::view_mut(&mut bytes)
            .unwrap()
            .edit()
            .fields()
            .name()
            .set(long),
        Err(Error::TooLong)
    );
    assert_eq!(bytes, before);
    let view = Holder::view_mut(&mut bytes).unwrap();
    view.set_name("a").unwrap();
    assert_eq!((view.name(), view.tail()), ("a", Some(7)));
}

#[test]
fn newtype_bounds_forward_through_chains_and_keep_declared_inner_bounds() {
    #[derive(ZeroPod)]
    struct Alias(Name);
    assert_eq!(Alias::encoded_len(&"long", &[3]), Err(Error::TooLong));
    assert_eq!(Alias::max_len(&[3]), Some(7));
    #[derive(ZeroPod)]
    struct Bounded(#[max_len(3)] String);
    assert_eq!(Bounded::encoded_len(&"four", &[10]), Err(Error::TooLong));
    assert_eq!(Bounded::max_len(&[10]), Some(7));
    let invalid = borsh::to_vec("four").unwrap();
    assert_eq!(Bounded::check(&invalid, &[10]), Err(Error::TooLong));
}
