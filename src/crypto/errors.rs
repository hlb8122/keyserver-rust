use std::fmt;

#[derive(Debug)]
pub enum CryptoError {
    Deserialization,
    Verification,
    NonHexAddress,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CryptoError::Deserialization => "deserialization error",
            CryptoError::Verification => "verification error",
            CryptoError::NonHexAddress => "hex decoding error",
        };
        write!(f, "{}", printable)
    }
}

impl From<hex::FromHexError> for CryptoError {
    fn from(_: hex::FromHexError) -> Self {
        CryptoError::NonHexAddress
    }
}
