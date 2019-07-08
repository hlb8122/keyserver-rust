use bitcoin_hashes::{
    hmac::{Hmac, HmacEngine},
    sha256, Hash, HashEngine,
};

pub fn generate_token(msg: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut engine = HmacEngine::<sha256::Hash>::new(secret);
    engine.input(msg);
    Hmac::<sha256::Hash>::from_engine(engine)[..].to_vec()
}

pub fn validate_token(msg: &[u8], secret: &[u8], expected: &[u8]) -> bool {
    generate_token(msg, secret) == expected
}
