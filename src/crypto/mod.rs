pub mod authentication;
pub mod ecdsa;
pub mod errors;
pub mod token;

use errors::CryptoError;

use bitcoin_hashes::{hash160::Hash as Hash160, Hash};
pub use bitcoincash_addr::*;

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

pub trait Addressable {
    fn to_raw_address(&self) -> Vec<u8>;
}

impl<P: PublicKey> Addressable for P {
    fn to_raw_address(&self) -> Vec<u8> {
        Hash160::hash(&self.serialize()).to_vec()
    }
}
