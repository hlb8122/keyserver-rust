use bitcoincash_addr::{Base58Error, CashAddrDecodingError};
use hyper::{Body, Response};
use prost::DecodeError;
use rocksdb::Error as RocksError;

pub enum PutError {}

pub enum GetError {
    Address(CashAddrDecodingError, Base58Error),
    DB(RocksError),
    Decode(DecodeError),
    NotFound,
}

impl Into<Response<Body>> for GetError {
    fn into(self) -> Response<Body> {
        match self {
            GetError::Address(cashaddr_err, base58_err) => Response::builder()
                .status(400)
                .body(Body::from(format!("{} and {}", cashaddr_err, base58_err)))
                .unwrap(),
            GetError::DB(_) => Response::builder().status(500).body(Body::empty()).unwrap(),
            GetError::Decode(err) => Response::builder()
                .status(400)
                .body(Body::from(err.to_string()))
                .unwrap(),
            GetError::NotFound => Response::builder().status(404).body(Body::empty()).unwrap(),
        }
    }
}

impl From<(CashAddrDecodingError, Base58Error)> for GetError {
    fn from((cash_err, base58_err): (CashAddrDecodingError, Base58Error)) -> Self {
        GetError::Address(cash_err, base58_err)
    }
}

impl From<RocksError> for GetError {
    fn from(err: RocksError) -> Self {
        GetError::DB(err)
    }
}

impl From<DecodeError> for GetError {
    fn from(err: DecodeError) -> Self {
        GetError::Decode(err)
    }
}
