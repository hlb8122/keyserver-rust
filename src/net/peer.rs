use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures::prelude::*;
use log::{error, warn};
use prost::Message;
use reqwest::{Client, Error as ReqError, Url};
use url::ParseError;

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address},
    db::KeyDB,
    models::address_metadata::{AddressMetadata, Payload},
    payments::VALID_DURATION,
};

use crate::bitcoin::tx_stream::StreamError;

#[derive(Debug)]
pub enum PeerError {
    UrlError(ParseError),
    ResponseError(ReqError),
    Decode,
}

impl From<ParseError> for PeerError {
    fn from(err: ParseError) -> PeerError {
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
    async fn get_metadata(&self, peer_url: &str, bitcoin_addr: &str) -> Result<Bytes, PeerError> {
        // Construct URL
        let url_str = format!("{}/keys/{}", peer_url, bitcoin_addr);
        let url = match Url::parse(&url_str) {
            Ok(ok) => ok,
            Err(e) => return Err(e.into()),
        };

        // Get response
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(PeerError::ResponseError)?;

        // Receive body
        response.bytes().await.map_err(PeerError::ResponseError)
    }

    pub async fn peer_polling(
        self,
        key_db: KeyDB,
        key_stream: impl Stream<Item = Result<(String, Address), StreamError>>,
    ) {
        key_stream
            .for_each(|res| {
                let client = self.clone();
                let key_db_inner = key_db.clone();
                async move {
                    let (peer_addr, bitcoin_addr) = match res {
                        Ok(ok) => ok,
                        Err(_) => return,
                    };
                    let bitcoin_addr_str = match bitcoin_addr.encode() {
                        Ok(ok) => ok,
                        Err(e) => {
                            warn!("{}", e);
                            return;
                        }
                    };

                    // Waiting period
                    let delay = tokio::time::delay_for(Duration::from_secs(VALID_DURATION));
                    delay.await;

                    // Get raw metadata from peer
                    let metadata_raw =
                        match client.get_metadata(&peer_addr, &bitcoin_addr_str).await {
                            Ok(ok) => ok,
                            Err(err) => {
                                warn!("{:?}", err);
                                return;
                            }
                        };

                    let metadata = match AddressMetadata::decode(&metadata_raw[..]) {
                        Ok(ok) => ok,
                        Err(err) => {
                            warn!("{:?}", err);
                            return;
                        }
                    };

                    // Check metadata
                    let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
                    metadata.encode(&mut metadata_raw).unwrap();
                    if let Err(e) = validate::<Secp256k1>(&bitcoin_addr, &metadata) {
                        warn!("peer supplied invalid metadata {:?}", e);
                        return;
                    }

                    let raw_payload = &metadata.serialized_payload;
                    let payload = match Payload::decode(&raw_payload[..]) {
                        Ok(ok) => ok,
                        Err(e) => {
                            warn!("peer supplied invalid payload {:?}", e);
                            return;
                        }
                    };

                    match key_db_inner.check_timestamp(&bitcoin_addr, &payload) {
                        Ok(Err(_)) => {
                            warn!("refusing to pull outdated metadata");
                            return;
                        }
                        Err(_) => {
                            error!("failed to check timestamp");
                            return;
                        }
                        _ => (),
                    }

                    if let Err(e) = key_db_inner.put(&bitcoin_addr, &metadata) {
                        error!("failed to put peer metadata {}", e);
                    };
                }
            })
            .await;
    }
}
