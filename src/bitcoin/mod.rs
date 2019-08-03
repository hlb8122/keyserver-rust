mod client;
pub mod tx_stream;

const PRICE: u64 = 5;

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use bitcoin::Transaction;
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
        let first_output = tx.output.get(0).unwrap(); // This is safe

        if !check_op_return(first_output.script_pubkey.as_bytes()) {
            return false;
        }

        if let Some(second_output) = tx.output.get(1).as_ref() {
            let value = second_output.value;
            if value != PRICE {
                return false;
            }

            // Check p2pkh addr
            let script = &second_output.script_pubkey[..];
            if let Some(pubkey_hash) = extract_pubkey_hash(script) {
                return self.0.read().unwrap().contains(&pubkey_hash);
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
}

pub fn check_op_return(script: &[u8]) -> bool {
    if script[0] != 0x6a {
        // Not op_return
        return false;
    }
    // OP_RETURN || keyserver || peer addr || bitcoin pk hash || metadata digest
    if script.len() != 1 + 9 + 6 + 20 + 20 {
        // Not correct length
        return false;
    }
    if &script[1..10] != KEYSERVER_PREFIX {
        // Not keyserver op_return
        return false;
    }
    true
}

fn extract_pubkey_hash(raw_script: &[u8]) -> Option<Vec<u8>> {
    if raw_script.len() != 25 {
        return None;
    }

    if raw_script[0..3] != [118, 169, 76] {
        return None;
    }

    if raw_script[23..25] != [136, 172] {
        return None;
    }

    Some(raw_script[3..23].to_vec())
}
