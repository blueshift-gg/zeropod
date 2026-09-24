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
