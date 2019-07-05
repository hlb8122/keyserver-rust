use bitcoin_hashes::{ripemd160, Hash};

use super::{ecdsa::Secp256k1PublicKey, *};

pub struct BitcoinAddressScheme;

impl AddressScheme for BitcoinAddressScheme {
    type PublicKey = Secp256k1PublicKey;
    type Address = BitcoinAddress;

    fn pubkey_to_addr(pk: &Self::PublicKey) -> BitcoinAddress {
        BitcoinAddress(ripemd160::Hash::hash(&pk.serialize())[..].to_vec())
    }
}

#[derive(PartialEq)]
pub struct BitcoinAddress(Vec<u8>);

impl Address for BitcoinAddress {
    fn serialize(&self) -> Vec<u8> {
        self.0.clone()
    }

    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError> {
        // TODO: Checks
        Ok(BitcoinAddress(raw.to_vec()))
    }
}
