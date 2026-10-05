/// Why bytes are not a valid encoding, or a value cannot be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes end before the encoding does.
    TooShort,
    /// A `bool` byte other than 0 or 1.
    InvalidBool,
    /// An `Option` tag other than 0 or 1, or an enum tag past its variants.
    InvalidTag,
    /// A string that is not UTF-8.
    InvalidUtf8,
    /// A string or vector longer than its `#[max_len]` or capacity.
    TooLong,
    /// No room left in the view for a field to grow into.
    NoRoom,
    /// Bytes left over after the encoding, for `from_slice`.
    TrailingBytes,
    /// A counted collection's element encoding has zero bytes.
    ZeroSizedItem,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::TooShort => "the bytes end before the encoding does",
            Self::InvalidBool => "a bool byte other than 0 or 1",
            Self::InvalidTag => "an option or enum tag out of range",
            Self::InvalidUtf8 => "a string that is not UTF-8",
            Self::TooLong => "a string or vector past its maximum length",
            Self::NoRoom => "no room for a field to grow into",
            Self::TrailingBytes => "bytes left after the encoding",
            Self::ZeroSizedItem => "a counted collection's element encoding has zero bytes",
        })
    }
}

impl core::error::Error for Error {}

#[cfg(feature = "solana-program-error")]
impl From<Error> for solana_program_error::ProgramError {
    fn from(error: Error) -> Self {
        use solana_program_error::ProgramError;
        match error {
            // A value too long for its field: the caller's to fix.
            Error::TooLong => ProgramError::InvalidArgument,
            Error::NoRoom => ProgramError::AccountDataTooSmall,
            _ => ProgramError::InvalidAccountData,
        }
    }
}
