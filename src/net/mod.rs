pub mod errors;
pub mod jsonrpc_client;
pub mod payments;
pub mod peer;

use bytes::BytesMut;
use futures::{future::Future, stream::Stream};
use prost::Message;

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address},
    db::Database,
    models::{AddressMetadata, Payload},
};

use errors::ServerError;
