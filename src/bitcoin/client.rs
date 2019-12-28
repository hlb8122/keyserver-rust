use crate::net::jsonrpc_client::*;

use std::sync::Arc;

use futures::Future;
use serde_json::Value;

#[derive(Clone)]
pub struct BitcoinClient(Arc<JsonClient>);

impl BitcoinClient {
    pub fn new(endpoint: String, username: String, password: String) -> BitcoinClient {
        BitcoinClient(Arc::new(JsonClient::new(endpoint, username, password)))
    }

    pub async fn get_new_addr(&self) -> Result<String, ClientError> {
        let request = self.0.build_request("getnewaddress".to_string(), vec![]);
        self.0.send_request(&request).await?.into_result::<String>()
    }

    pub async fn send_tx(&self, raw_tx: &[u8]) -> Result<String, ClientError> {
        let request = self.0.build_request(
            "sendrawtransaction".to_string(),
            vec![Value::String(hex::encode(raw_tx))],
        );

        self.0.send_request(&request).await?.into_result::<String>()
    }
}
