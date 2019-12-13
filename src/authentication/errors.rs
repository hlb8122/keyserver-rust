use std::fmt;

use crate::crypto::errors::CryptoError;

#[derive(Debug)]
pub enum ValidationError {
    KeyType,
    Preimage,
    Outdated,
    ExpiredTTL,
    Crypto(CryptoError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ValidationError::KeyType => "bad key type",
            ValidationError::Preimage => "digest mismatch",
            ValidationError::Outdated => "metadata is outdated",
            ValidationError::ExpiredTTL => "expired TTL",
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
