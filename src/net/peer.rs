use bytes::BytesMut;
use futures::{
    future::{err, Either},
    Future, Stream,
};
use prost::Message;
use reqwest::{r#async::Client, Error as ReqError, Url, UrlError};

use crate::models::AddressMetadata;

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

pub struct PeerClient {
    client: Client,
}

impl PeerClient {
    pub fn new() -> PeerClient {
        PeerClient {
            client: Client::new(),
        }
    }

    pub fn get_metadata(
        &self,
        peer_addr: &str,
        addr: &str,
    ) -> impl Future<Item = AddressMetadata, Error = PeerError> + Send {
        // Construct URL
        let url_str = format!("http://{}/keys/{}", peer_addr, addr);
        let url = match Url::parse(&url_str) {
            Ok(ok) => ok,
            Err(e) => return Either::B(err(e.into())),
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
}
