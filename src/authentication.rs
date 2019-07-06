use crate::crypto::*;
use crate::models::AddressMetadata;

use bitcoin_hashes::{sha256, Hash};
use prost::Message;

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

pub fn validate<AS: AddressScheme, S>(
    addr: &AS::Address,
    metadata: &AddressMetadata,
) -> Result<(), ValidationError>
where
    S: SigScheme<PublicKey = AS::PublicKey>,
{
    let meta_pk = S::PublicKey::deserialize(&metadata.pub_key).map_err(|e| e.into())?;

    // Check preimage
    if AS::pubkey_to_addr(&meta_pk) == *addr {
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
