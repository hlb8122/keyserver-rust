pub mod bitcoin_addr;
pub mod ecdsa;

pub enum CryptoError {
    Deserialization,
    Verification,
}

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

pub trait Address
where
    Self: Sized,
    Self: PartialEq,
{
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError>;
}

pub trait AddressScheme {
    type PublicKey: PublicKey;
    type Address: Address;

    fn pubkey_to_addr(pk: &Self::PublicKey) -> Self::Address;
}
