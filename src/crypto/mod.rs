pub mod bitcoin_addr;
pub mod ecdsa;

pub enum CryptoError {
    Deserialization,
    Verification,
    NonHexAddress,
}

impl From<hex::FromHexError> for CryptoError {
    fn from(_err: hex::FromHexError) -> Self {
        CryptoError::NonHexAddress
    }
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

pub trait Hexable
where
    Self: Sized,
{
    fn from_hex(hex: String) -> Result<Self, CryptoError>;
    fn into_hex(self) -> String;
}

impl<U: Address> Hexable for U {
    fn from_hex(hex: String) -> Result<Self, CryptoError> {
        let addr_raw = hex::decode(hex)?;
        Address::deserialize(&addr_raw)
    }

    fn into_hex(self) -> String {
        hex::encode(self.serialize())
    }
}

pub trait AddressScheme {
    type PublicKey: PublicKey;
    type Address: Address;

    fn pubkey_to_addr(pk: &Self::PublicKey) -> Self::Address;
}
