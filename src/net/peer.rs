use std::time::{Duration, Instant};

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

// Duration between hearing Keyserver tx and fetching
const FETCH_WAIT: u64 = 60;

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
    client: Client,
}

impl Default for PeerClient {
    fn default() -> PeerClient {
        PeerClient {
            client: Client::new(),
        }
    }
}

impl PeerClient {
    fn get_metadata(
        &self,
        peer_url: &str,
        bitcoin_addr: &str,
    ) -> impl Future<Item = AddressMetadata, Error = PeerError> + Send {
        // Construct URL
        let url_str = format!("{}/keys/{}", peer_url, bitcoin_addr);
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
                .and_then(|body| AddressMetadata::decode(body).map_err(|_| PeerError::Decode)),
        )
    }

    pub fn peer_polling(
        &self,
        key_db: KeyDB,
        key_stream: impl Stream<Item = (String, Address), Error = StreamError> + Send,
    ) -> impl Future<Item = (), Error = ()> + Send {
        let client = self.to_owned();
        key_stream
            .for_each(move |(peer_addr, bitcoin_addr)| {
                let bitcoin_addr_str = match bitcoin_addr.encode() {
                    Ok(ok) => ok,
                    Err(e) => {
                        warn!("{}", e);
                        return Either::A(future::ok(()));
                    }
                };

                // Get metadata from peer
                let metadata_fut = client.get_metadata(&peer_addr, &bitcoin_addr_str);

                // Waiting period
                let delay =
                    tokio_timer::Delay::new(Instant::now() + Duration::from_secs(FETCH_WAIT));
                let delayed_meta_fut = delay.then(|_| metadata_fut);

                let key_db_inner = key_db.clone();
                Either::B(delayed_meta_fut.then(move |metadata| {
                    let metadata = match metadata {
                        Ok(ok) => ok,
                        Err(e) => {
                            warn!("{:?}", e);
                            return future::ok(());
                        }
                    };

                    // Check metadata
                    let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
                    metadata.encode(&mut metadata_raw).unwrap();
                    if let Err(e) = validate::<Secp256k1>(&bitcoin_addr, &metadata) {
                        warn!("peer supplied invalid metadata {:?}", e);
                        return future::ok(());
                    }

                    match key_db_inner.check_timestamp(&bitcoin_addr, &metadata) {
                        Ok(Err(_)) => {
                            warn!("refusing to pull outdated metadata");
                            return future::ok(());
                        }
                        Err(_) => {
                            warn!("failed to check timestamp");
                            return future::ok(());
                        }
                        _ => (),
                    }

                    if let Err(e) = key_db_inner.put(&bitcoin_addr, &metadata) {
                        error!("failed to put peer metadata {}", e);
                    };
                    future::ok(())
                }))
            })
            .map_err(|e| error!("{:?}", e))
    }
}
