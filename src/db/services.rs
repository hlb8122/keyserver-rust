use std::pin::Pin;

use bitcoincash_addr::Address;
use futures_core::{
    task::{Context, Poll},
    Future,
};
use hyper::{Body, Response};
use prost::Message as _;
use tower_service::Service;

use super::{errors::*, Database};

#[derive(Clone)]
pub struct MetadataGetter {
    db: Database,
}

impl MetadataGetter {
    pub fn new(db: Database) -> Self {
        MetadataGetter { db }
    }
}

impl Service<String> for MetadataGetter {
    type Response = Vec<u8>;
    type Error = GetError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, addr_str: String) -> Self::Future {
        let db_inner = self.db.clone();
        let fut = async move {
            // Convert address
            let addr = Address::decode(&addr_str)?;

            // Grab metadata from DB
            let metadata = db_inner.get(&addr)?.ok_or(GetError::NotFound)?;

            // Encode metadata as hex
            let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
            metadata.encode(&mut raw_payload).unwrap();

            // Respond
            Ok(raw_payload)
        };
        Box::pin(fut)
    }
}

pub struct MetadataPutter {
    db: Database,
}

impl MetadataPutter {
    pub fn new(db: Database) -> Self {
        MetadataPutter { db }
    }
}

impl Service<String, > for MetadataPutter {
    type Response = ();
    type Error = PutError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
