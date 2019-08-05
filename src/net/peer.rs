use std::sync::Arc;

use bitcoin_hashes::{hash160, Hash};
use bytes::BytesMut;
use futures::{
    future::{self, Either},
    Future, Stream,
};
use log::{error, warn};
use prost::Message;
use reqwest::{r#async::Client, Error as ReqError, Url, UrlError};

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address},
    db::KeyDB,
    models::AddressMetadata,
};

use crate::bitcoin::tx_stream::StreamError;

#[derive(Debug)]
pub enum PeerError {
    UrlError(UrlError),
    ResponseError(ReqError),
    Decode,
}

impl From<UrlError> for PeerError {
    fn from(err: UrlError) -> PeerError {
        PeerError::UrlError(err)
    }
}

#[derive(Clone)]
pub struct PeerClient {
    client: Arc<Client>,
}

impl Default for PeerClient {
    fn default() -> PeerClient {
        PeerClient {
            client: Arc::new(Client::new()),
        }
    }
}

impl PeerClient {
    fn get_metadata(
        &self,
        peer_host: &str,
        bitcoin_addr: &str,
    ) -> impl Future<Item = AddressMetadata, Error = PeerError> + Send {
        // Construct URL
        let url_str = format!("http://{}/keys/{}", peer_host, bitcoin_addr);
        let url = match Url::parse(&url_str) {
            Ok(ok) => ok,
            Err(e) => return Either::B(future::err(e.into())),
        };

        // Get then decode response
        Either::A(
            self.client
                .get(url)
                .send()
                .map_err(PeerError::ResponseError)
                .and_then(|resp| {
                    // Receive body
                    resp.into_body().map_err(PeerError::ResponseError).fold(
                        BytesMut::new(),
                        move |mut body, chunk| {
                            body.extend_from_slice(&chunk);
                            Ok::<_, PeerError>(body)
                        },
                    )
                })
                .and_then(|body| Message::decode(body).map_err(|_| PeerError::Decode)), // Decode protobuf message
        )
    }

    pub fn peer_polling(
        &self,
        key_db: KeyDB,
        key_stream: impl Stream<Item = (String, Address, Vec<u8>), Error = StreamError> + Send,
    ) -> impl Future<Item = (), Error = ()> + Send {
        let client = self.clone();
        key_stream
            .for_each(move |(peer_addr, bitcoin_addr, meta_digest)| {
                let bitcoin_addr_str = match bitcoin_addr.encode() {
                    Ok(ok) => ok,
                    Err(e) => {
                        warn!("{}", e);
                        return Either::A(future::ok(()));
                    }
                };

                // Get metadata from peer
                let metadata_fut = client.get_metadata(&peer_addr, &bitcoin_addr_str);

                let key_db_inner = key_db.clone();
                Either::B(metadata_fut.then(move |metadata| {
                    let metadata = match metadata {
                        Ok(ok) => ok,
                        Err(e) => {
                            warn!("{:?}", e);
                            return future::ok(());
                        }
                    };

                    // Check digest matches
                    let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
                    metadata.encode(&mut metadata_raw).unwrap();
                    let actual_digest = &hash160::Hash::hash(&metadata_raw)[..];
                    if actual_digest != &meta_digest[..] {
                        warn!("found fraudulent metadata");
                        return future::ok(());
                    }

                    if let Err(e) = validate::<Secp256k1>(&bitcoin_addr, &metadata) {
                        warn!("peer supplied invalid metadata {:?}", e);
                        return future::ok(());
                    }

                    if let Err(e) = key_db_inner.clone().put(&bitcoin_addr, &metadata) {
                        error!("failed to put peer metadata {}", e);
                    };
                    future::ok(())
                }))
            })
            .map_err(|e| error!("{:?}", e))
    }
}
