//! The derive's output depends on nothing the deriving crate can shadow, and
//! finds zeropod through `#[zeropod(crate = path)]`.
#![allow(dead_code, non_camel_case_types)]

mod framework {
    pub use zeropod as reexported;
}

mod user {
    // Each shadows a name generated code could otherwise reach for.
    mod core {}
    mod zeropod {}
    struct Ok;
    struct Err;
    struct Some;
    struct None;
    struct Option;
    struct Result;

    use crate::framework::reexported::ZeroPod;

    #[derive(ZeroPod)]
    #[zeropod(crate = crate::framework::reexported)]
    pub struct Record {
        pub amount: u64,
        #[max_len(8)]
        pub name: ::std::string::String,
        pub kind: Kind,
    }

    #[derive(ZeroPod)]
    #[zeropod(crate = crate::framework::reexported)]
    pub enum Kind {
        Plain,
        Named(#[max_len(4)] ::std::string::String),
    }
}

#[test]
fn derives_compile_beside_shadowing_names_through_a_reexport() {
    use zeropod::Layout;

    let bytes = [&7u64.to_le_bytes()[..], &[2, 0, 0, 0], b"hi", &[0]].concat();
    let record = user::Record::view(&bytes).unwrap();
    assert_eq!((record.amount(), record.name()), (7, "hi"));
    // Exhaustive without the hidden variant, which can hold no value.
    let kind = match record.kind() {
        user::KindRef::Plain => "plain",
        user::KindRef::Named(name) => name,
    };
    assert_eq!(kind, "plain");
}

#[test]
fn raw_and_underscore_field_setters_and_tuple_indices() {
    use zeropod::{Layout, ZeroPod};
    #[derive(ZeroPod)]
    struct Names {
        r#type: u8,
        _x: u8,
        x: u8,
    }
    #[derive(ZeroPod)]
    struct Pair(u8, u8);
    let mut bytes = [0; 3];
    let view = Names::view_mut(&mut bytes).unwrap();
    view.set_type(1).unwrap();
    view.set__x(2).unwrap();
    view.set_x(3).unwrap();
    assert_eq!((view.r#type(), view._x(), view.x()), (1, 2, 3));
    view.edit().fields().r#type().set(4).unwrap();
    let mut bytes = [0; 2];
    let view = Pair::view_mut(&mut bytes).unwrap();
    view.set_0(5).unwrap();
    view.set_1(6).unwrap();
    assert_eq!((view._0(), view._1()), (5, 6));
}

#[test]
fn boxed_recursive_struct_round_trips() {
    #[derive(Debug, PartialEq, zeropod::ZeroPod)]
    struct Node {
        value: u8,
        next: Option<Box<Node>>,
    }
    let node = Node {
        value: 1,
        next: Some(Box::new(Node {
            value: 2,
            next: None,
        })),
    };
    let bytes = zeropod::to_vec(&node).unwrap();
    assert_eq!(zeropod::from_slice::<Node>(&bytes).unwrap(), node);
}

#[test]
fn enum_field_bindings_do_not_shadow_generated_locals() {
    #[derive(Debug, PartialEq, zeropod::ZeroPod, borsh::BorshSerialize)]
    enum Event {
        Transfer {
            len: u32,
            at: u16,
            out: u8,
            bytes: String,
            value: Option<u32>,
            __zeropod_field_0: u8,
        },
        Tuple(u8, String),
        Unit,
    }
    for value in [
        Event::Transfer {
            len: 17,
            at: 4,
            out: 2,
            bytes: "payload".into(),
            value: Some(9),
            __zeropod_field_0: 7,
        },
        Event::Tuple(3, "tuple".into()),
        Event::Unit,
    ] {
        let bytes = zeropod::to_vec(&value).unwrap();
        assert_eq!(bytes, borsh::to_vec(&value).unwrap());
        assert_eq!(zeropod::from_slice::<Event>(&bytes).unwrap(), value);
        assert_eq!(zeropod::read::<Event>(&bytes).unwrap().to_owned(), value);
    }
}

#[test]
fn getters_win_over_view_convenience_methods() {
    use zeropod::{Edit, Layout, ZeroPod};
    #[derive(Debug, PartialEq, ZeroPod)]
    struct Names {
        edit: u8,
        to_owned: u8,
        size: u8,
    }
    let value = Names {
        edit: 1,
        to_owned: 2,
        size: 3,
    };
    let mut bytes = zeropod::to_vec(&value).unwrap();
    let view = Names::view_mut(&mut bytes).unwrap();
    assert_eq!((view.edit(), view.to_owned(), view.size()), (1, 2, 3));
    assert_eq!(Names::size(view), 3);
    assert_eq!(Names::own(view), value);
    Edit::<Names>::view(&mut bytes)
        .unwrap()
        .fields()
        .edit()
        .set(4)
        .unwrap();
    assert_eq!(Names::view(&bytes).unwrap().edit(), 4);
}

#[test]
fn self_bounds_refer_to_the_original_generic_struct() {
    use zeropod::{Error, Layout, ZeroPod};
    #[derive(Debug, PartialEq, ZeroPod)]
    struct Named<const N: usize> {
        #[max_len(Self::CAP)]
        name: String,
    }
    impl<const N: usize> Named<N> {
        const CAP: usize = N;
    }
    assert_eq!(Named::<3>::max_len(&[]), Some(7));
    let mut bytes = [0; 16];
    let view = Named::<3>::view_mut(&mut bytes).unwrap();
    view.set_name("abc").unwrap();
    assert_eq!(view.name(), "abc");
    assert_eq!(view.set_name("long"), Err(Error::TooLong));
    assert_eq!(view.edit().fields().name().set("long"), Err(Error::TooLong));
    view.edit().fields().name().set("xy").unwrap();
    assert_eq!(view.name(), "xy");
    let invalid = borsh::to_vec("long").unwrap();
    assert!(matches!(Named::<3>::view(&invalid), Err(Error::TooLong)));
}
