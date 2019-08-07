pub mod base58;
pub mod cashaddr;

use bitcoin_hashes::{ripemd160, Hash};

use crate::{
    bitcoin::Network,
    crypto::{errors::CryptoError, PublicKey},
    SETTINGS,
};

pub use base58::Base58Codec;
pub use cashaddr::CashAddrCodec;

#[derive(PartialEq, Clone)]
pub enum AddressScheme {
    Base58,
    CashAddr,
}

#[derive(PartialEq)]
pub struct Address {
    pub scheme: AddressScheme,
    payload: Vec<u8>,
}

impl<'a> AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.payload
    }
}

impl Address {
    pub fn new(payload: Vec<u8>, scheme: AddressScheme) -> Self {
        Address { payload, scheme }
    }

    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }

    pub fn encode(&self) -> Result<String, CryptoError> {
        match self.scheme {
            AddressScheme::CashAddr => CashAddrCodec::encode(&self.payload, &SETTINGS.network),
            AddressScheme::Base58 => Base58Codec::encode(&self.payload, &SETTINGS.network),
        }
    }

    pub fn decode(input: &str) -> Result<Self, CryptoError> {
        CashAddrCodec::decode(input, &SETTINGS.network)
            .or_else(|_| Base58Codec::decode(input, &SETTINGS.network))
    }
}

pub trait Addressable {
    fn to_raw_address(&self) -> Vec<u8>;
}

impl<P: PublicKey> Addressable for P {
    fn to_raw_address(&self) -> Vec<u8> {
        ripemd160::Hash::hash(&self.serialize()).to_vec()
    }
}

pub trait AddressCodec {
    fn encode(raw: &[u8], network: &Network) -> Result<String, CryptoError>;

    fn decode(s: &str, network: &Network) -> Result<Address, CryptoError>;
}
