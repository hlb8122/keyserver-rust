use crate::crypto::Key;
use crate::models::AddressMetadata;
use prost::Message;
use rocksdb::{Error, DB};

const DB_PATH: &str = "./db";

struct KeyDB(DB);

impl KeyDB {
    fn try_new(path: &str) -> Result<Self, Error> {
        DB::open_default(path).map(|db| KeyDB(db))
    }

    fn try_default() -> Result<Self, Error> {
        Self::try_new(DB_PATH)
    }

    fn close(self) {
        drop(self)
    }

    fn put(&self, key: impl Key, metadata: AddressMetadata) -> Result<(), Error> {
        let mut raw_metadata = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut raw_metadata).unwrap();
        self.0.put(&key.get_address(), raw_metadata)
    }

    fn get(&self, key: impl Key) -> Result<Option<AddressMetadata>, Error> {
        // This panics if stored bytes are fucked
        self.0
            .get(&key.get_address())
            .map(|opt_dat| opt_dat.map(|dat| AddressMetadata::decode(&dat[..]).unwrap()))
    }
}
