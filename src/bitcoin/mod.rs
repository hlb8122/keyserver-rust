mod client;
pub mod tx_stream;

const PRICE: u64 = 5;

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use bitcoin::Transaction;

use crate::{
    crypto::{Address, AddressScheme},
    models::Output,
};

pub use client::BitcoinClient;

const KEYSERVER_PREFIX: &[u8; 9] = b"keyserver";

#[derive(Clone)]
pub enum Network {
    Mainnet = 0,
    Testnet = 1,
}

impl Into<String> for Network {
    fn into(self) -> String {
        match self {
            Network::Mainnet => "main".to_string(),
            Network::Testnet => "test".to_string(),
        }
    }
}

#[derive(Default, Clone)]
pub struct WalletState(Arc<RwLock<HashSet<Vec<u8>>>>);

impl WalletState {
    pub fn add(&self, addr: Vec<u8>) {
        self.0.write().unwrap().insert(addr);
    }

    pub fn remove(&self, addr: Vec<u8>) {
        self.0.write().unwrap().remove(&addr);
    }

    pub fn check_outputs(&self, tx: Transaction) -> bool {
        // TODO: Enforce op_return outputs

        // Check first output
        let first_output = tx.output.get(0).unwrap();
        let value = first_output.value;
        if value != PRICE {
            return false;
        }

        // Check p2pkh addr
        let script = &first_output.script_pubkey[..];
        if let Some(pubkey_hash) = extract_pubkey_hash(script) {
            // Check if wallet contains that address
            if self.0.read().unwrap().contains(&pubkey_hash) {
                // Flush address
                self.0.write().unwrap().remove(&pubkey_hash);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

pub fn extract_op_return(script: &[u8]) -> Option<(String, Address, Vec<u8>)> {
    if script[0] != 106 {
        // Not op_return
        return None;
    }

    // OP_RETURN || keyserver || bitcoin pk hash || metadata digest || peer host
    if script.len() <= 1 + 9 + 20 + 20 {
        // Too short
        return None;
    }

    if &script[1..10] != KEYSERVER_PREFIX {
        // Not keyserver op_return
        return None;
    }

    // Parse bitcoin address
    let bitcoin_addr_raw = script[10..30].to_vec();
    let bitcoin_addr = Address::new(bitcoin_addr_raw, AddressScheme::Base58);

    // Parse metaaddress digest
    let meta_digest = script[30..50].to_vec();

    // Parse host
    let raw_host = &script[50..];
    let host = match std::str::from_utf8(raw_host) {
        Ok(ok) => ok.to_string(),
        Err(_) => return None,
    };

    Some((host, bitcoin_addr, meta_digest))
}

fn extract_pubkey_hash(raw_script: &[u8]) -> Option<Vec<u8>> {
    if raw_script.len() != 25 {
        return None;
    }

    if raw_script[0..3] != [118, 169, 20] {
        return None;
    }

    if raw_script[23..25] != [136, 172] {
        return None;
    }

    Some(raw_script[3..23].to_vec())
}

pub fn generate_outputs(raw_addr: Vec<u8>) -> Vec<Output> {
    // Generate p2pkh
    let p2pkh_script_pre: [u8; 3] = [118, 169, 20];
    let p2pkh_script_post: [u8; 2] = [136, 172];
    let p2pkh_script = [&p2pkh_script_pre[..], &raw_addr[..], &p2pkh_script_post[..]].concat();
    let p2pkh_output = Output {
        amount: Some(PRICE),
        script: p2pkh_script,
    };

    vec![p2pkh_output]
}
