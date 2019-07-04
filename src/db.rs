use crate::crypto::Address;
use crate::models::AddressMetadata;
use prost::Message;
use rocksdb::{Error, DB};

const DB_PATH: &str = "./db";

struct KeyDB(DB);

impl KeyDB {
    fn try_new(path: &str) -> Result<Self, Error> {
        DB::open_default(path).map(KeyDB)
    }

    fn try_default() -> Result<Self, Error> {
        Self::try_new(DB_PATH)
    }

    fn close(self) {
        drop(self)
    }

    fn put(&self, addr: &impl Address, metadata: &AddressMetadata) -> Result<(), Error> {
        let mut raw_metadata = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut raw_metadata).unwrap();
        self.0.put(addr.serialize(), raw_metadata)
    }

    fn get(&self, addr: &impl Address) -> Result<Option<AddressMetadata>, Error> {
        // This panics if stored bytes are fucked
        self.0
            .get(addr.serialize())
            .map(|opt_dat| opt_dat.map(|dat| AddressMetadata::decode(&dat[..]).unwrap()))
    }

    fn is_recent(&self, addr: &impl Address, metadata: &AddressMetadata) -> Result<bool, Error> {
        let old_metadata_opt = self.get(addr)?;
        match old_metadata_opt {
            Some(old_metadata) => match (metadata.payload.as_ref(), old_metadata.payload) {
                (Some(new_payload), Some(old_payload)) => {
                    Ok(new_payload.timestamp > old_payload.timestamp)
                }
                (_, None) => Ok(true),
                (None, Some(_)) => Ok(false),
            },
            None => Ok(false),
        }
    }
}
