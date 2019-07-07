pub mod cashaddr;

use bitcoin_hashes::{ripemd160, Hash};

use super::{errors::CryptoError, PublicKey};

#[derive(Clone)]
pub enum Network {
    Mainnet = 0,
    Testnet = 1,
}

#[derive(PartialEq, Clone)]
pub enum AddressScheme {
    Base54,
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
    pub fn encode(&self) -> Result<String, CryptoError> {
        match self.scheme {
            AddressScheme::CashAddr => {
                cashaddr::CashAddrCodec::encode(&self.payload, Network::Mainnet)
            }
            AddressScheme::Base54 => unreachable!(),
        }
    }

    pub fn decode(input: String) -> Result<Self, CryptoError> {
        cashaddr::CashAddrCodec::decode(input, Network::Mainnet)
    }
}

pub trait Addressable {
    fn to_addr(&self, scheme: AddressScheme) -> Address;
}

impl<P: PublicKey> Addressable for P {
    fn to_addr(&self, scheme: AddressScheme) -> Address {
        let payload = match scheme {
            AddressScheme::Base54 => ripemd160::Hash::hash(&self.serialize()).to_vec(),
            _ => unreachable!(),
        };

        Address { scheme, payload }
    }
}

pub trait AddressCodec {
    fn encode(raw: &[u8], network: Network) -> Result<String, CryptoError>;

    fn decode(s: String, network: Network) -> Result<Address, CryptoError>;
}
