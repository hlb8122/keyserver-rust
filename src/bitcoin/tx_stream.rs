use std::{
    convert::TryInto,
    net::{Ipv4Addr, SocketAddrV4},
};

use bitcoin::{consensus::encode, util::psbt::serialize::Deserialize, Transaction};
use bitcoin_zmq::{errors::SubscriptionError, Topic, ZMQSubscriber};
use futures::{Future, Stream};

use crate::crypto::{Address, AddressScheme};

use super::check_op_return;

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
) -> impl Stream<Item = (String, Address, Vec<u8>), Error = StreamError> {
    stream.filter_map(|tx| {
        // This unwrap is safe due to tx originating from deserialization
        let output = tx.output.get(0).unwrap();

        // Check for op return script
        let script = output.script_pubkey.as_bytes();
        if !check_op_return(script) {
            return None;
        }

        // Parse peer addr
        let peer_ip_raw: [u8; 4] = script[10..14].try_into().unwrap();
        let peer_port_raw: [u8; 2] = script[14..16].try_into().unwrap();
        let peer_ip = Ipv4Addr::from(peer_ip_raw);
        let peer_port = u16::from_be_bytes(peer_port_raw);
        let peer_addr_str = SocketAddrV4::new(peer_ip, peer_port).to_string();

        // Parse bitcoin address
        let bitcoin_addr_raw = script[16..36].to_vec();
        let bitcoin_addr = Address::new(bitcoin_addr_raw, AddressScheme::Base58);

        // Parse metaaddress digest
        let meta_digest = script[36..56].to_vec();
        Some((peer_addr_str, bitcoin_addr, meta_digest))
    })
}
