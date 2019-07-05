use prost::Message;
use tower_web::*;

use crate::{db::KeyDB, crypto::bitcoin_addr::BitcoinAddress, crypto::Address};

struct RestInterface(KeyDB);

impl_web! {
    impl RestInterface {
        #[get("/keys/:key")]
        fn get(&self, addr_str: String) -> Result<String, ()> {
            let addr_raw = hex::decode(addr_str).map_err(|_| ())?;
            let addr = BitcoinAddress::deserialize(&addr_raw).map_err(|_| ())?;
            match self.0.get(&addr).map_err(|_| ())? {
                Some(some) => {
                    let mut raw_payload = Vec::with_capacity(some.encoded_len());
                    some.encode(&mut raw_payload).unwrap();
                    Ok(hex::encode(raw_payload))
                    },
                None => Err(())
            }
        }

        
    }
}

