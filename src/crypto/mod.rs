pub mod ecdsa;

pub enum CryptoError {
    Deserialization,
    Verification,
}

pub trait PrivateKey {
    fn get_address<A: Address>(&self) -> A;
}

pub trait PublicKey
where
    Self: Sized,
{
    fn to_address<A: Address>(&self) -> A;
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
    Self: From<Vec<u8>>,
    Self: PartialEq,
{
    fn serialize(&self) -> Vec<u8>;
}
