mod address;
pub mod ecdsa;
pub mod errors;

use errors::CryptoError;

pub use address::*;

pub trait PublicKey
where
    Self: Sized,
{
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError>;
}

pub trait Signature
where
    Self: Sized,
{
    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError>;
}

pub trait SigScheme {
    type PublicKey: PublicKey;
    type Signature: Signature;

    fn verify(msg: &[u8], key: &Self::PublicKey, sig: &Self::Signature) -> Result<(), CryptoError>;
}
