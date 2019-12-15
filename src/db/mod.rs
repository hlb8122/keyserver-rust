pub mod errors;
pub mod services;

use std::sync::Arc;

use bitcoincash_addr::Address;
use cashweb_protobuf::address_metadata::{AddressMetadata, Payload};
use prost::Message;
use rocksdb::{CompactionDecision, Error, Options, DB};

use crate::authentication::expired;

fn ttl_filter(_level: u32, _key: &[u8], value: &[u8]) -> CompactionDecision {
    // This panics if the bytes stored are fucked
    let metadata = AddressMetadata::decode(value).unwrap();
    let payload = Payload::decode(&metadata.serialized_payload[..]).unwrap();
    if expired(&payload) {
        // Payload has expired
        CompactionDecision::Remove
    } else {
        CompactionDecision::Keep
    }
}

#[derive(Clone)]
pub struct Database(Arc<DB>);

impl Database {
    pub fn try_new(path: &str) -> Result<Self, Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compaction_filter("ttl", ttl_filter);

        DB::open(&opts, &path).map(Arc::new).map(Database)
    }

    pub fn close(self) {
        drop(self)
    }

    pub fn put(&self, addr: &Address, metadata: &AddressMetadata) -> Result<(), Error> {
        let mut raw_metadata = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut raw_metadata).unwrap();
        self.0.put(addr.as_body(), raw_metadata)
    }

    pub fn get(&self, addr: &Address) -> Result<Option<AddressMetadata>, Error> {
        // This panics if stored bytes are fucked
        let metadata_res = self
            .0
            .get(addr.as_body())
            .map(|opt_dat| opt_dat.map(|dat| AddressMetadata::decode(&dat[..]).unwrap()));

        if let Ok(Some(metadata)) = &metadata_res {
            let raw_payload = &metadata.serialized_payload;
            let payload = Payload::decode(&raw_payload[..]).unwrap();
            if expired(&payload) {
                self.0.delete(addr.as_body())?;
                Ok(None)
            } else {
                metadata_res
            }
        } else {
            metadata_res
        }
    }
}
