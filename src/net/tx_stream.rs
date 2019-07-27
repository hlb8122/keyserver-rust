use bitcoin::{util::psbt::serialize::Deserialize, Transaction};
use bitcoin_zmq::{errors::SubscriptionError, Topic, ZMQSubscriber};
use futures::{Future, Stream};

pub enum StreamError {
    Subscription(SubscriptionError),
    Deserialization,
}

impl From<SubscriptionError> for StreamError {
    fn from(err: SubscriptionError) -> StreamError {
        StreamError::Subscription(err)
    }
}

pub fn get_tx_stream(
    addr: &str,
) -> (impl Future<Item = (), Error = StreamError>
         + Send + Sized, impl Stream<Item=Transaction, Error=()>) {
    let (stream, broker) = ZMQSubscriber::single_stream(addr, Topic::RawTx, 256);
    let stream = stream
        .and_then(move |raw_tx| {
            let tx = Transaction::deserialize(&raw_tx)
                .map_err(|_| ())?;
            Ok(tx)
        });

    (broker.map_err(StreamError::Subscription), stream)
}
