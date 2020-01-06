use bitcoin::{consensus::encode, util::psbt::serialize::Deserialize, Transaction};
use bitcoin_zmq::{
    errors::{SubscriptionError, ZMQError},
    ZMQListener,
};
use futures::prelude::*;

use crate::crypto::Address;

use super::extract_op_return;

#[derive(Debug)]
pub enum StreamError {
    Subscription(SubscriptionError),
    Deserialization(encode::Error),
}

impl From<SubscriptionError> for StreamError {
    fn from(err: SubscriptionError) -> StreamError {
        StreamError::Subscription(err)
    }
}

pub async fn get_tx_stream(
    node_addr: &str,
) -> Result<impl Stream<Item = Result<Transaction, StreamError>>, ZMQError> {
    let stream = ZMQListener::bind(node_addr).await?.stream();
    let stream = stream
        .map_err(StreamError::Subscription)
        .and_then(move |raw_tx| {
            async move { Transaction::deserialize(&raw_tx).map_err(StreamError::Deserialization) }
        });

    Ok(stream)
}

// Extract peer address, bitcoin address and metadata digest from tx stream
pub fn extract_details(
    stream: impl Stream<Item = Result<Transaction, StreamError>>,
) -> impl Stream<Item = Result<(String, Address), StreamError>> {
    stream.try_filter_map(|tx| {
        async move {
            Ok(tx
                .output
                .iter()
                .map(|output| output.script_pubkey.as_bytes())
                .find_map(extract_op_return))
        }
    })
}
