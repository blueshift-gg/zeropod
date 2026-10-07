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
                    .edit()
                    .fields()
                    .tags()
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

macro_rules! bounded_map {
    ($test:ident, $map:ty) => {
        #[test]
        fn $test() {
            #[derive(ZeroPod, borsh::BorshSerialize)]
            struct Names {
                #[max_len(4, 2)]
                names: $map,
            }
            let good = Names {
                names: [("a".into(), "bb".into())].into(),
            };
            assert_eq!(Names::max_len(&[]), Some(4 + 4 * (6 + 6)));
            let mut bytes = zeropod::to_vec(&good).unwrap();
            bytes.resize(Names::max_len(&[]).unwrap(), 0xa5);
            let before = bytes.clone();
            for pair in [("toolong", "bb"), ("a", "toolong")] {
                let bad = Names {
                    names: [(pair.0.into(), pair.1.into())].into(),
                };
                assert_eq!(zeropod::to_vec(&bad), Err(Error::TooLong));
                assert_eq!(
                    Names::check(&borsh::to_vec(&bad).unwrap(), &[]),
                    Err(Error::TooLong)
                );
                assert_eq!(
                    Names::view_mut(&mut bytes).unwrap().set_names(&bad.names),
                    Err(Error::TooLong)
                );
                assert_eq!(bytes, before);
                assert_eq!(
                    Names::view_mut(&mut bytes)
                        .unwrap()
                        .edit()
                        .fields()
                        .names()
                        .set(&bad.names),
                    Err(Error::TooLong)
                );
                assert_eq!(bytes, before);
            }
            let replacement: $map = [("cc".into(), "dd".into()), ("ee".into(), "ff".into())].into();
            let view = Names::view_mut(&mut bytes).unwrap();
            view.set_names(&replacement).unwrap();
            let used = view.size();
            assert_eq!(
                zeropod::from_slice::<Names>(&bytes[..used]).unwrap().names,
                replacement
            );
        }
    };
}

bounded_map!(btree_map_checks_key_and_value_bounds, std::collections::BTreeMap<String, String>);
#[cfg(feature = "std")]
bounded_map!(hash_map_checks_key_and_value_bounds, std::collections::HashMap<String, String>);
