use crate::net::jsonrpc_client::*;

use futures::Future;
use serde_json::Value;

pub struct BitcoinClient(JsonClient);

impl BitcoinClient {
    pub fn get_new_addr(&mut self) -> Box<Future<Item = String, Error = ClientError> + Send> {
        let request = self.0.build_request("getnewaddress".to_string(), vec![]); // TODO: Add to wallet
        Box::new(
            self.0
                .send_request(&request)
                .and_then(|resp| resp.into_result::<String>()),
        )
    }

    pub fn send_tx(
        &mut self,
        raw_tx: String,
    ) -> Box<Future<Item = String, Error = ClientError> + Send> {
        let request = self.0.build_request(
            "sendrawtransaction".to_string(),
            vec![Value::String(raw_tx)],
        );
        Box::new(
            self.0
                .send_request(&request)
                .and_then(|resp| resp.into_result::<String>()),
        )
    }
}
