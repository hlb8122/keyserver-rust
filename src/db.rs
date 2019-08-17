use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::crypto::Address;
use crate::models::AddressMetadata;
use prost::Message;
use rocksdb::{Error, DB};

use crate::net::errors::ValidationError;

#[derive(Clone)]
pub struct KeyDB(Arc<DB>);

impl KeyDB {
    pub fn try_new(path: &str) -> Result<Self, Error> {
        DB::open_default(path).map(Arc::new).map(KeyDB)
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
        self.0
            .get(&addr)
            .map(|opt_dat| opt_dat.map(|dat| AddressMetadata::decode(&dat[..]).unwrap()))
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

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                if new_payload.timestamp + new_payload.ttl < timestamp {
                    // Proposed timestamp is in excess of TTL
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
