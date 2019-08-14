use super::*;

pub struct Secp256k1PublicKey(pub secp256k1::PublicKey);

impl PublicKey for Secp256k1PublicKey {
    fn serialize(&self) -> Vec<u8> {
        self.0.serialize_uncompressed().to_vec()
    }

    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError> {
        secp256k1::PublicKey::from_slice(raw)
            .map(Secp256k1PublicKey)
            .map_err(|_| CryptoError::PubkeyDeserialization)
    }
}

pub struct Secp256k1Sig(pub secp256k1::Signature);

impl Signature for Secp256k1Sig {
    fn deserialize(raw: &[u8]) -> Result<Self, CryptoError> {
        secp256k1::Signature::from_compact(raw)
            .map(Secp256k1Sig)
            .map_err(|_| CryptoError::SigDeserialization)
    }
}

pub struct Secp256k1 {}

impl SigScheme for Secp256k1 {
    type PublicKey = Secp256k1PublicKey;
    type Signature = Secp256k1Sig;

    fn verify(msg: &[u8], key: &Self::PublicKey, sig: &Self::Signature) -> Result<(), CryptoError> {
        let msg = secp256k1::Message::from_slice(msg).unwrap();
        secp256k1::Secp256k1::verification_only()
            .verify(&msg, &sig.0, &key.0)
            .map_err(|_| CryptoError::Verification)
    }
}
