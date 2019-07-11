use bitcoin_hashes::{
    hmac::{Hmac, HmacEngine},
    sha256, Hash, HashEngine,
};

use secp256k1::rand::Rng;

pub fn generate_secret(len: usize) -> Vec<u8> {
    let mut rng = secp256k1::rand::thread_rng();
    (0..len).map(|_| rng.gen::<u8>()).collect()
}

pub fn generate_token(msg: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut engine = HmacEngine::<sha256::Hash>::new(secret);
    engine.input(msg);
    Hmac::<sha256::Hash>::from_engine(engine)[..].to_vec()
}

pub fn validate_token(msg: &[u8], secret: &[u8], expected: &[u8]) -> bool {
    generate_token(msg, secret) == expected
}

mod tests {
    use super::*;

    #[test]
    fn test_validate() {
        let secret = generate_secret(16);
        let msg = &b"DEADBEEF"[..];

        let token = generate_token(msg, &secret);

        assert!(validate_token(msg, &secret, &token))
    }

    #[test]
    fn test_validate_wrong_sig() {
        let secret_a = generate_secret(16);
        let secret_b = generate_secret(16);
        let msg = &b"DEADBEEF"[..];

        let token = generate_token(msg, &secret_a);

        assert!(!validate_token(msg, &secret_b, &token))
    }

    #[test]
    fn test_validate_wrong_msg() {
        let secret = generate_secret(16);
        let msg_a = &b"DEADBEEF"[..];
        let msg_b = &b"BEDEAD"[..];

        let token = generate_token(msg_a, &secret);

        assert!(!validate_token(msg_b, &secret, &token))
    }

    #[test]
    fn test_validate_wrong_token() {
        let secret = generate_secret(16);
        let msg = &b"DEADBEEF"[..];

        let token = &b"BEEFEED"[..];

        assert!(!validate_token(msg, &secret, &token))
    }
}
