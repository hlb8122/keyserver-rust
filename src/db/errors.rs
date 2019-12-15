use bitcoincash_addr::{Base58Error, CashAddrDecodingError};
use hyper::{Body, Error as HyperError, Response};
use prost::DecodeError;
use rocksdb::Error as RocksError;

use crate::authentication::errors::ValidationError;

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

pub enum PutError {
    Address(CashAddrDecodingError, Base58Error),
    DB(RocksError),
    Buffer(HyperError),
    MetadataDecode(prost::DecodeError),
    Outdated,
    PayloadDecode(prost::DecodeError),
    UnsupportedSigScheme,
    Validation(ValidationError),
}

impl From<(CashAddrDecodingError, Base58Error)> for PutError {
    fn from((cash_err, base58_err): (CashAddrDecodingError, Base58Error)) -> Self {
        PutError::Address(cash_err, base58_err)
    }
}

impl From<RocksError> for PutError {
    fn from(err: RocksError) -> Self {
        PutError::DB(err)
    }
}


impl Into<Response<Body>> for PutError {
    fn into(self) -> Response<Body> {
        match self {
            PutError::Address(cashaddr_err, base58_err) => Response::builder()
                .status(400)
                .body(Body::from(format!("{} and {}", cashaddr_err, base58_err))),
            PutError::DB(_) => Response::builder().status(500).body(Body::empty()),
            // TODO: Rest of them
            _ => Response::builder().status(404).body(Body::empty()),
        }
        .unwrap()
    }
}
