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

    pub fn get_new_addr(&mut self) -> Box<dyn Future<Item = String, Error = ClientError> + Send> {
        let request = self.0.build_request("getnewaddress".to_string(), vec![]);
        Box::new(
            self.0
                .send_request(&request)
                .and_then(|resp| resp.into_result::<String>()),
        )
    }

    pub fn send_tx(
        &self,
        raw_tx: &[u8],
    ) -> Box<dyn Future<Item = String, Error = ClientError> + Send> {
        let request = self.0.build_request(
            "sendrawtransaction".to_string(),
            vec![Value::String(hex::encode(raw_tx))],
        );
        Box::new(
            self.0
                .send_request(&request)
                .and_then(|resp| resp.into_result::<String>()),
        )
    }
}
