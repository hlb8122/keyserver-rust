use std::fmt;

#[derive(Debug)]
pub enum CryptoError {
    PubkeyDeserialization,
    SigDeserialization,
    Verification,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CryptoError::PubkeyDeserialization => "invalid pubkey",
            CryptoError::SigDeserialization => "invalid signature",
            CryptoError::Verification => "verification failed",
        };
        write!(f, "{}", printable)
    }
}
