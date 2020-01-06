use json_rpc::{clients::http::HttpConnector, prelude::*};

use std::sync::Arc;

use serde_json::Value;

#[derive(Clone)]
pub struct BitcoinClient<C>(Arc<HttpClient<C>>);

impl BitcoinClient<HttpConnector> {
    pub fn new(endpoint: String, username: String, password: String) -> Self {
        BitcoinClient(Arc::new(HttpClient::new(
            endpoint,
            Some(username),
            Some(password),
        )))
    }
}

impl<C> std::ops::Deref for BitcoinClient<C> {
    type Target = HttpClient<C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum BitcoinError {
    Http(HttpError),
    Rpc(RpcError),
    Json(JsonError),
    EmptyResponse,
}

impl<C> BitcoinClient<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    pub async fn get_new_addr(&self) -> Result<String, BitcoinError> {
        let request = self
            .build_request()
            .method("getnewaddress")
            .finish()
            .unwrap();
        let response = self.send(request).await.map_err(BitcoinError::Http)?;
        if response.is_error() {
            return Err(BitcoinError::Rpc(response.error().unwrap()));
        }
        response
            .into_result()
            .ok_or(BitcoinError::EmptyResponse)?
            .map_err(BitcoinError::Json)
    }

    pub async fn send_tx(&self, raw_tx: &[u8]) -> Result<String, BitcoinError> {
        let request = self
            .build_request()
            .method("sendrawtransaction")
            .params(vec![Value::String(hex::encode(raw_tx))])
            .finish()
            .unwrap();
        let response = self.send(request).await.map_err(BitcoinError::Http)?;
        if response.is_error() {
            return Err(BitcoinError::Rpc(response.error().unwrap()));
        }
        response
            .into_result()
            .ok_or(BitcoinError::EmptyResponse)?
            .map_err(BitcoinError::Json)
    }
}
