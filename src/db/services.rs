use std::pin::Pin;

use bitcoincash_addr::Address;
use cashweb_protobuf::address_metadata::{AddressMetadata, Payload};
use futures_core::{
    task::{Context, Poll},
    Future,
};
use hyper::{body, Body};
use prost::Message as _;
use tower_service::Service;

use super::{errors::*, Database};
use crate::{
    authentication::{errors::ValidationError, validate},
    crypto::ecdsa::*,
};

impl Service<String> for Database {
    type Response = Vec<u8>;
    type Error = GetError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, addr_str: String) -> Self::Future {
        let db_inner = self.clone();
        let fut = async move {
            // Convert address
            let addr = Address::decode(&addr_str)?;

            // Grab metadata from DB
            let metadata = db_inner.get(&addr)?.ok_or(GetError::NotFound)?;

            // Serialize metadata
            let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
            metadata.encode(&mut raw_payload).unwrap();

            // Respond
            Ok(raw_payload)
        };
        Box::pin(fut)
    }
}

impl Service<(String, Body)> for Database {
    type Response = ();
    type Error = PutError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (addr_str, body): (String, Body)) -> Self::Future {
        let db_inner = self.clone();
        let fut = async move {
            // Convert address
            let addr = Address::decode(&addr_str)?;

            // Decode metadata
            let body_raw = body::aggregate(body).await.map_err(PutError::Buffer)?;
            let metadata = AddressMetadata::decode(body_raw).map_err(PutError::MetadataDecode)?;

            // Grab metadata from DB
            match metadata.scheme {
                1 => validate::<Secp256k1>(&addr, &metadata).map_err(PutError::Validation)?,
                _ => return Err(PutError::UnsupportedSigScheme),
            }

            // Decode payload
            let raw_payload = &metadata.serialized_payload[..];
            let payload = Payload::decode(raw_payload).map_err(PutError::PayloadDecode)?;

            if let Some(old_metadata) = db_inner.get(&addr)? {
                // This panics if stored bytes are malformed
                let old_payload = Payload::decode(&old_metadata.serialized_payload[..]).unwrap();
                if payload.timestamp < old_payload.timestamp {
                    // Timestamp is outdated
                    return Err(PutError::Outdated);
                } // TODO: Check if = and use lexicographical
            }

            // Put to database
            db_inner.put(&addr, &metadata)?;

            // Respond
            Ok(())
        };
        Box::pin(fut)
    }
}
