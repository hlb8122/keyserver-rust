use crate::crypto::{errors::CryptoError, *};
use crate::models::AddressMetadata;

use bitcoin_hashes::{sha256, Hash};
use prost::Message;

#[derive(Debug)]
pub enum ValidationError {
    KeyType,
    Preimage,
    EmptyPayload,
    Crypto(CryptoError),
}

impl Into<ValidationError> for CryptoError {
    fn into(self) -> ValidationError {
        ValidationError::Crypto(self)
    }
}

pub fn validate<S: SigScheme>(
    addr: &Address,
    metadata: &AddressMetadata,
) -> Result<(), ValidationError> {
    let meta_pk = S::PublicKey::deserialize(&metadata.pub_key).map_err(|e| e.into())?;

    // Check preimage
    if &*meta_pk.to_raw_address() == addr.as_ref() {
        return Err(ValidationError::Preimage);
    }

    // Check signature
    let payload = metadata
        .payload
        .as_ref()
        .ok_or(ValidationError::EmptyPayload)?;
    let mut raw_payload = Vec::with_capacity(payload.encoded_len());
    payload.encode(&mut raw_payload).unwrap();
    let payload_digest = &sha256::Hash::hash(&raw_payload)[..];
    let sig = S::Signature::deserialize(&metadata.signature).map_err(|e| e.into())?;

    S::verify(payload_digest, &meta_pk, &sig).map_err(|e| e.into())
}
