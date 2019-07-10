mod jsonrpc_client;

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
