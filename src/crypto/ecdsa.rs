use bitcoin_hashes::{ripemd160, Hash};

use super::*;

struct Secp256k1PublicKey(secp256k1::PublicKey);

impl PublicKey for Secp256k1PublicKey {
    fn get_address(&self) -> Vec<u8> {
        // TODO: Convert to script address
        ripemd160::Hash::hash(&self.0.serialize())[..].to_vec()
    }

    fn serialize(&self) -> Vec<u8> {
        self.0.serialize().to_vec()
    }
}

struct Secp256k1Sig(secp256k1::Signature);

impl Signature for Secp256k1Sig {}

struct Secp256k1 {
    context: secp256k1::Secp256k1<secp256k1::VerifyOnly>
}

impl Default for Secp256k1 {
    fn default() -> Self {
        Secp256k1 {
            context: secp256k1::Secp256k1::verification_only()
        }
    }
}

impl SigScheme for Secp256k1 {
    type PublicKey = Secp256k1PublicKey;
    type Signature = Secp256k1Sig;

    fn verify(&self, msg: &[u8], key: &Self::PublicKey, sig: &Self::Signature) -> bool {
        let msg = secp256k1::Message::from_slice(msg).unwrap();
        self.context
            .verify(&msg, &sig.0, &key.0)
            .is_ok()
    }
}
