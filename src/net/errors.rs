use std::fmt;

use actix_web::{error, HttpResponse};
use prost::DecodeError;
use rocksdb::Error as RocksError;

use crate::crypto::errors::CryptoError;

#[derive(Debug)]
pub enum ServerError {
    DB(RocksError),
    Crypto(CryptoError),
    NotFound,
    MetadataDecode,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ServerError::DB(err) => return err.fmt(f),
            ServerError::Crypto(err) => return err.fmt(f),
            ServerError::NotFound => "not found",
            ServerError::MetadataDecode => "metadata decoding error",
        };
        write!(f, "{}", printable)
    }
}

impl From<CryptoError> for ServerError {
    fn from(err: CryptoError) -> Self {
        ServerError::Crypto(err)
    }
}

impl From<DecodeError> for ServerError {
    fn from(_: DecodeError) -> Self {
        ServerError::MetadataDecode
    }
}

impl From<RocksError> for ServerError {
    fn from(err: RocksError) -> Self {
        ServerError::DB(err)
    }
}

impl error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServerError::DB(_) => HttpResponse::InternalServerError().body("database failure"),
            ServerError::NotFound => HttpResponse::NotFound().body("missing key address"),
            ServerError::MetadataDecode => HttpResponse::NotFound().body("malformed metadata"),
            ServerError::Crypto(err) => match err {
                CryptoError::Deserialization => HttpResponse::BadRequest().body("invalid address"),
                CryptoError::Decoding => HttpResponse::BadRequest().body("address decoding failed"),
                CryptoError::Encoding => {
                    HttpResponse::InternalServerError().body("address encoding failed")
                }
                CryptoError::Verification => HttpResponse::BadRequest().body("validation failed"),
            },
        }
    }
}
