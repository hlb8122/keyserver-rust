pub mod ecdsa;

pub trait PrivateKey {
    fn get_address(&self) -> Vec<u8>;
}

pub trait PublicKey {
    fn get_address(&self) -> Vec<u8>;
    fn serialize(&self) -> Vec<u8>;
}

pub trait Signature {}

pub trait SigScheme {
    type PublicKey: PublicKey;
    type Signature: Signature;

    fn verify(&self, msg: &[u8], key: &Self::PublicKey, sig: &Self::Signature) -> bool;
}
