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
            CryptoError::Deserialization => "invalid address",
            CryptoError::Verification => "verification failed",
            CryptoError::Decoding => "address decoding failed",
            CryptoError::Encoding => "address encoding failed",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Debug)]
pub enum ValidationError {
    KeyType,
    Preimage,
    EmptyPayload,
    Crypto(CryptoError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ValidationError::KeyType => "bad key type",
            ValidationError::Preimage => "digest mismatch",
            ValidationError::EmptyPayload => "empty payload",
            ValidationError::Crypto(err) => return err.fmt(f),
        };
        write!(f, "{}", printable)
    }
}

impl Into<ValidationError> for CryptoError {
    fn into(self) -> ValidationError {
        ValidationError::Crypto(self)
    }
}
