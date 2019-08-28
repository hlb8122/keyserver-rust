use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    crypto::Address,
    models::{AddressMetadata, Payload},
};
use prost::Message;
use rocksdb::{CompactionDecision, Error, Options, DB};

use crate::net::errors::ValidationError;

fn expired(payload: &Payload) -> bool {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    payload.timestamp + payload.ttl < timestamp
}

fn ttl_filter(_level: u32, _key: &[u8], value: &[u8]) -> CompactionDecision {
    // This panics if the bytes stored are fucked
    let metadata = AddressMetadata::decode(value).unwrap();
    let payload = metadata.payload.unwrap();
    if expired(&payload) {
        // Payload has expired
        CompactionDecision::Remove
    } else {
        CompactionDecision::Keep
    }
}

#[derive(Clone)]
pub struct KeyDB(Arc<DB>);

impl KeyDB {
    pub fn try_new(path: &str) -> Result<Self, Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compaction_filter("ttl", ttl_filter);

        DB::open(&opts, &path).map(Arc::new).map(KeyDB)
    }

    pub fn close(self) {
        drop(self)
    }

    pub fn put(&self, addr: &Address, metadata: &AddressMetadata) -> Result<(), Error> {
        let mut raw_metadata = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut raw_metadata).unwrap();
        self.0.put(&addr, raw_metadata)
    }

    pub fn get(&self, addr: &Address) -> Result<Option<AddressMetadata>, Error> {
        // This panics if stored bytes are fucked
        let metadata_res = self
            .0
            .get(&addr)
            .map(|opt_dat| opt_dat.map(|dat| AddressMetadata::decode(&dat[..]).unwrap()));

        if let Ok(Some(metadata)) = &metadata_res {
            if expired(metadata.payload.as_ref().unwrap()) {
                self.0.delete(&addr)?;
                Ok(None)
            } else {
                metadata_res
            }
        } else {
            metadata_res
        }
    }

    pub fn check_timestamp(
        &self,
        addr: &Address,
        metadata: &AddressMetadata,
    ) -> Result<Result<(), ValidationError>, Error> {
        if let Some(old_metadata) = self.get(addr)? {
            if let (Some(new_payload), Some(old_payload)) =
                (metadata.payload.as_ref(), old_metadata.payload)
            {
                if new_payload.timestamp < old_payload.timestamp {
                    // Timestamp is outdated
                    return Ok(Err(ValidationError::Outdated));
                } // TODO: Check if = and use lexicographical

                if expired(new_payload) {
                    // Payload has expired
                    return Ok(Err(ValidationError::ExpiredTTL));
                }
            } else {
                // Payload is empty
                return Ok(Err(ValidationError::EmptyPayload));
            }
        }
        Ok(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use secp256k1::{rand, Secp256k1};

    use crate::crypto::{ecdsa::Secp256k1PublicKey, *};

    use super::*;

    #[test]
    fn test_ttl_ok() {
        // Open DB
        let key_db = KeyDB::try_new("./test_db/ttl_ok").unwrap();

        // Generate metadata with 10 sec TTL
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let payload = Payload {
            timestamp,
            ttl: 10,
            entries: vec![],
        };
        let metadata = AddressMetadata {
            pub_key: vec![],
            payload: Some(payload),
            signature: vec![],
            scheme: 1,
        };

        // Generate address
        let secp = Secp256k1::new();
        let (_, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let public_key = Secp256k1PublicKey(pk);
        let addr = Address {
            body: public_key.to_raw_address(),
            ..Default::default()
        };

        // Put to database
        key_db.put(&addr, &metadata).unwrap();

        // Get from database before TTL
        assert!(key_db.get(&addr).unwrap().is_some());

        // Wait until TTL is over
        std::thread::sleep(std::time::Duration::from_secs(12));

        // Force compactification
        key_db.0.compact_range::<&[u8], &[u8]>(None, None);

        // Get from database after TTL
        assert!(key_db.get(&addr).unwrap().is_none());
    }
}
