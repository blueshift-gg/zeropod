use std::collections::BTreeSet;
use zeropod::{Error, Layout, ZeroPod};

macro_rules! bounded_set {
    ($test:ident, $set:ty) => {
        #[test]
        fn $test() {
            #[derive(Debug, PartialEq, ZeroPod)]
            struct Tags {
                #[max_len(10, 4)]
                tags: $set,
                tail: Option<u32>,
            }
            let good = Tags {
                tags: ["four".into()].into(),
                tail: Some(42),
            };
            let bad = Tags {
                tags: ["toolong".into()].into(),
                tail: Some(99),
            };
            assert_eq!(zeropod::to_vec(&bad), Err(Error::TooLong));
            let mut bytes = zeropod::to_vec(&good).unwrap();
            assert_eq!(zeropod::from_slice::<Tags>(&bytes).unwrap(), good);
            assert!(bytes.len() <= Tags::max_len(&[]).unwrap());
            bytes.resize(Tags::max_len(&[]).unwrap(), 0xa5);
            let before = bytes.clone();
            assert_eq!(
                Tags::view_mut(&mut bytes).unwrap().set_tags(&bad.tags),
                Err(Error::TooLong)
            );
            assert_eq!(bytes, before);
            assert_eq!(
                Tags::view_mut(&mut bytes)
                    .unwrap()
                    .tags_mut()
                    .set(&bad.tags),
                Err(Error::TooLong)
            );
            assert_eq!(bytes, before);
            assert_eq!(zeropod::write(&bad, &mut bytes), Err(Error::TooLong));
            assert_eq!(bytes, before);
            let view = Tags::view_mut(&mut bytes).unwrap();
            let replacement: $set = ["a".into(), "four".into()].into();
            view.set_tags(&replacement).unwrap();
            let used = view.size();
            assert_eq!(
                zeropod::from_slice::<Tags>(&bytes[..used]).unwrap(),
                Tags {
                    tags: replacement,
                    tail: Some(42)
                }
            );
        }
    };
}

bounded_set!(
    btree_set_checks_inner_bounds_before_writing,
    BTreeSet<String>
);
#[cfg(feature = "std")]
bounded_set!(
    hash_set_checks_inner_bounds_before_writing,
    std::collections::HashSet<String>
);
