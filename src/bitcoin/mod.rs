pub mod tx_stream;

use std::string::ToString;

use serde::Deserialize;

use crate::{crypto::Address, SETTINGS};

const KEYSERVER_PREFIX: &[u8; 9] = b"keyserver";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum Network {
    Mainnet = 0,
    Testnet = 1,
    Regnet = 2,
}

impl From<bitcoincash_addr::Network> for Network {
    fn from(network: bitcoincash_addr::Network) -> Network {
        match network {
            bitcoincash_addr::Network::Main => Network::Mainnet,
            bitcoincash_addr::Network::Test => Network::Testnet,
            bitcoincash_addr::Network::Regtest => Network::Regnet,
        }
    }
}

impl Into<bitcoincash_addr::Network> for Network {
    fn into(self) -> bitcoincash_addr::Network {
        match self {
            Network::Mainnet => bitcoincash_addr::Network::Main,
            Network::Testnet => bitcoincash_addr::Network::Test,
            Network::Regnet => bitcoincash_addr::Network::Regtest,
        }
    }
}

impl ToString for Network {
    fn to_string(&self) -> String {
        match self {
            Network::Mainnet => "mainnet".to_string(),
            Network::Testnet => "testnet".to_string(),
            Network::Regnet => "regnet".to_string(),
        }
    }
}

pub fn extract_op_return(script: &[u8]) -> Option<(String, Address)> {
    // OP_RETURN || LEN || keyserver || bitcoin pk hash || peer host
    if script.len() <= 2 + 9 + 20 {
        // Too short
        return None;
    }

    if script[0] != 106 {
        // Not op_return
        return None;
    }

    if script[1] as usize != script.len() - 2 {
        // Not length
        return None;
    }

    if &script[2..11] != KEYSERVER_PREFIX {
        // Not keyserver op_return
        return None;
    }

    // Parse host
    let raw_host = &script[31..];
    let url = match std::str::from_utf8(raw_host) {
        Ok(ok) => ok.to_string(),
        Err(_) => return None,
    };

    // Don't get from ourselves
    // TODO: This is super crude
    if url == format!("http://{}", SETTINGS.bind) {
        return None;
    }

    // Parse bitcoin address
    let bitcoin_addr_raw = script[11..31].to_vec();
    let bitcoin_addr = Address {
        body: bitcoin_addr_raw,
        network: SETTINGS.network.clone().into(),
        ..Default::default()
    };
    Some((url, bitcoin_addr))
}

pub fn generate_tx_data(base_url: &str, put_pk_hash: Vec<u8>) -> Vec<u8> {
    let raw_base_url = base_url.as_bytes();
    [
        &[106, 9 + 20 + base_url.len() as u8][..],
        &KEYSERVER_PREFIX[..],
        &put_pk_hash[..],
        raw_base_url,
    ]
    .concat()
}
