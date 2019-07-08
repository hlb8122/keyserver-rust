use std::fmt;

#[derive(Debug)]
pub enum CryptoError {
    Deserialization,
    Verification,
    Decoding,
    Encoding,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CryptoError::Deserialization => "deserialization error",
            CryptoError::Verification => "verification error",
            CryptoError::Decoding => "address decoding error",
            CryptoError::Encoding => "address encoding error",
        };
        write!(f, "{}", printable)
    }
}
