//! Each way bytes can fail to be a valid encoding is caught when viewed.

use zeropod::{Error, ZeroPod};

#[derive(ZeroPod, Debug, PartialEq)]
struct Sample {
    flag: bool,
    #[max_len(4)]
    name: String,
    choice: Option<u8>,
    shape: Shape,
}

#[derive(ZeroPod, Debug, PartialEq)]
enum Shape {
    Dot,
    Line(u8),
}

/// `flag`, `name` ("ab"), `choice` (None), `shape` (Dot).
const VALID: [u8; 9] = [1, 2, 0, 0, 0, b'a', b'b', 0, 0];

fn with(index: usize, byte: u8) -> Vec<u8> {
    let mut bytes = VALID.to_vec();
    bytes[index] = byte;
    bytes
}

#[test]
fn the_sample_is_valid() {
    let value = Sample {
        flag: true,
        name: "ab".into(),
        choice: None,
        shape: Shape::Dot,
    };
    assert_eq!(zeropod::from_slice::<Sample>(&VALID), Ok(value));
}

#[test]
fn every_invalid_encoding_is_rejected() {
    let read = |bytes: &[u8]| zeropod::read::<Sample>(bytes).map(|_| ());
    assert_eq!(read(&VALID[..8]), Err(Error::TooShort));
    assert_eq!(read(&with(0, 2)), Err(Error::InvalidBool));
    assert_eq!(read(&with(1, 5)), Err(Error::TooLong));
    assert_eq!(read(&with(5, 0xff)), Err(Error::InvalidUtf8));
    assert_eq!(read(&with(7, 2)), Err(Error::InvalidTag));
    assert_eq!(read(&with(8, 2)), Err(Error::InvalidTag));
    let mut long = with(1, 4);
    long.truncate(7);
    assert_eq!(read(&long), Err(Error::TooShort));
}

#[test]
fn from_slice_takes_all_the_bytes() {
    let bytes = [VALID.as_slice(), &[0]].concat();
    assert_eq!(
        zeropod::from_slice::<Sample>(&bytes),
        Err(Error::TrailingBytes)
    );
    assert!(zeropod::read::<Sample>(&bytes).is_ok());
}
