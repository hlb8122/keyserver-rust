use bitcoin_hashes::sha256d::Hash as Sha256d;
use rust_base58::base58::{FromBase58, ToBase58};

use super::*;

use crate::{bitcoin::Network, crypto::errors::CryptoError};

pub struct Base58Codec;

impl AddressCodec for Base58Codec {
    fn encode(raw: &[u8], network: &Network) -> Result<String, CryptoError> {
        let addr_type_byte = match network {
            Network::Mainnet => 0x00,
            Network::Testnet => 0x6f,
            Network::Regnet => 0x6f,
        };

        let mut body = Vec::with_capacity(raw.len() + 5);
        body.push(addr_type_byte);
        body.extend(raw);

        let checksum = Sha256d::hash(&body);
        body.extend(&checksum[0..4]);
        Ok(body.to_base58())
    }

    fn decode(s: &str, network: &Network) -> Result<Address, CryptoError> {
        // Convert from base58
        let v = s.from_base58().map_err(|_| CryptoError::Decoding)?;
        if v.len() < 6 {
            return Err(CryptoError::Decoding);
        }

        // Verify checksum
        let v0 = &v[0..v.len() - 4];
        let v1 = &v[v.len() - 4..v.len()];
        let cs = Sha256d::hash(v0);
        if v1[0] != cs[0] || v1[1] != cs[1] || v1[2] != cs[2] || v1[3] != cs[3] {
            return Err(CryptoError::Decoding);
        }

        // Check network byte
        let net_byte = match network {
            Network::Mainnet => 0x00,
            Network::Testnet => 0x6f,
            Network::Regnet => 0x6f,
        };
        if v0[0] != net_byte {
            return Err(CryptoError::Decoding);
        };

        // Extract hash160 address and return
        if v0.len() != 21 {
            return Err(CryptoError::Decoding);
        }

        let mut hash160addr = vec![0; 20];
        hash160addr.clone_from_slice(&v0[1..]);
        Ok(Address {
            scheme: AddressScheme::Base58,
            payload: hash160addr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_hashes::hash160::Hash as Hash160;
    use hex;

    #[test]
    fn to_legacyaddr() {
        let pubkey_hex = "04005937fd439b3c19014d5f328df8c7ed514eaaf41c1980b8aeab461dffb23fbf3317e42395db24a52ce9fc947d9c22f54dc3217c8b11dfc7a09c59e0dca591d3";
        let pubkeyhash = Hash160::hash(&hex::decode(pubkey_hex).unwrap()).to_vec();
        let legacyaddr = Base58Codec::encode(&pubkeyhash, &Network::Mainnet).unwrap();
        assert!(legacyaddr == "1NM2HFXin4cEQRBLjkNZAS98qLX9JKzjKn");
    }

    #[test]
    fn from_legacyaddr() {
        let legacyaddr = "1NM2HFXin4cEQRBLjkNZAS98qLX9JKzjKn";
        let result = Base58Codec::decode(legacyaddr, &Network::Mainnet).unwrap();
        let hash160 = result.as_ref();
        assert!(hex::encode(hash160) == "ea2407829a5055466b27784cde8cf463167946bf");
    }

    #[test]
    fn from_legacyaddr_errors() {
        assert!(Base58Codec::decode("0", &Network::Mainnet).is_err());
        assert!(
            Base58Codec::decode("1000000000000000000000000000000000", &Network::Mainnet).is_err()
        );
    }
}
