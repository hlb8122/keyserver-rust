use bitcoin::{consensus::encode, util::psbt::serialize::Deserialize, Transaction};
use bitcoin_zmq::{errors::SubscriptionError, Topic, ZMQSubscriber};
use futures::{Future, Stream};

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

pub fn get_tx_stream(
    node_addr: &str,
) -> (
    impl Stream<Item = Transaction, Error = StreamError>,
    impl Future<Item = (), Error = StreamError> + Send + Sized,
) {
    let (stream, broker) = ZMQSubscriber::single_stream(node_addr, Topic::RawTx, 256);
    let stream = stream
        .map_err(|_| unreachable!()) // TODO: Double check that this is safe
        .and_then(move |raw_tx| {
            Transaction::deserialize(&raw_tx).map_err(StreamError::Deserialization)
        });

    (stream, broker.map_err(StreamError::Subscription))
}

// Extract peer address, bitcoin address and metadata digest from tx stream
pub fn extract_details(
    stream: impl Stream<Item = Transaction, Error = StreamError>,
) -> impl Stream<Item = (String, Address), Error = StreamError> {
    stream.filter_map(|tx| {
        tx.output
            .iter()
            .map(|output| output.script_pubkey.as_bytes())
            .find_map(extract_op_return)
    })
}
