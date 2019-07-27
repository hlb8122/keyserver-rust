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
            ValidationError::KeyType => "key type error",
            ValidationError::Preimage => "preimage error",
            ValidationError::EmptyPayload => "empty payload error",
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
