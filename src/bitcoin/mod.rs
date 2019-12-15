pub const PRICE: u64 = 5;

use std::string::ToString;

use serde::Deserialize;

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
