//! On any bytes, what zeropod accepts Borsh reads the same way.

mod common;

use borsh::BorshDeserialize;
use common::{Profile, profile};
use proptest::prelude::*;

proptest! {
    #[test]
    fn corrupted_encodings_are_read_as_borsh_reads_them(
        value in profile(),
        flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..4),
        cut in any::<prop::sample::Index>(),
    ) {
        let mut bytes = borsh::to_vec(&value).unwrap();
        for (index, byte) in flips {
            let index = index.index(bytes.len());
            bytes[index] = byte;
        }
        bytes.truncate(cut.index(bytes.len() + 1));

        if let Ok(len) = <Profile as zeropod::ZeroPod>::check(&bytes, &[]) {
            let ours = zeropod::from_slice::<Profile>(&bytes[..len]).unwrap();
            let theirs = Profile::try_from_slice(&bytes[..len]).unwrap();
            prop_assert_eq!(ours, theirs);
        }
    }
}
