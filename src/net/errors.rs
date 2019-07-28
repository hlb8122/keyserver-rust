use std::fmt;

use actix_web::{error, HttpResponse};
use prost::DecodeError;
use rocksdb::Error as RocksError;

use crate::crypto::errors::{CryptoError, ValidationError};

#[derive(Debug)]
pub enum ServerError {
    DB(RocksError),
    Validation(ValidationError),
    Crypto(CryptoError),
    NotFound,
    MetadataDecode,
    Payment(PaymentError),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ServerError::DB(err) => return err.fmt(f),
            ServerError::Crypto(err) => return err.fmt(f),
            ServerError::NotFound => "not found",
            ServerError::MetadataDecode => "metadata decoding error",
            ServerError::Payment(err) => return err.fmt(f),
            ServerError::Validation(err) => return err.fmt(f),
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

impl error::ResponseError for CryptoError {
    fn error_response(&self) -> HttpResponse {
        match self {
            CryptoError::Deserialization => HttpResponse::BadRequest().body("invalid address"),
            CryptoError::Decoding => HttpResponse::BadRequest().body("address decoding failed"),
            CryptoError::Encoding => {
                HttpResponse::InternalServerError().body("address encoding failed")
            }
            CryptoError::Verification => HttpResponse::BadRequest().body("validation failed"),
        }
    }
}

impl error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServerError::Validation(err) => match err {
                ValidationError::Crypto(err_inner) => err_inner.error_response(),
                ValidationError::EmptyPayload => HttpResponse::BadRequest().body("empty payload"),
                ValidationError::KeyType => HttpResponse::BadRequest().body("bad key type"),
                ValidationError::Preimage => HttpResponse::BadRequest().body("digest mismatch"),
            },
            ServerError::DB(_) => HttpResponse::InternalServerError().body("database failure"),
            ServerError::NotFound => HttpResponse::NotFound().body("missing key address"),
            ServerError::MetadataDecode => HttpResponse::BadRequest().body("invalid metadata"),
            ServerError::Crypto(err) => err.error_response(),
            ServerError::Payment(err) => match err {
                PaymentError::Accept => HttpResponse::NotAcceptable().body("not acceptable"),
                PaymentError::Content => {
                    HttpResponse::UnsupportedMediaType().body("invalid content-type")
                }
                PaymentError::NoMerchantDat => HttpResponse::BadRequest().body("no merchant data"),
                PaymentError::Payload => {
                    HttpResponse::BadRequest().body("failed to receive payload")
                }
                PaymentError::Decode => HttpResponse::BadRequest().body("failed to decode body"),
                PaymentError::InvalidMerchantDat => {
                    HttpResponse::BadRequest().body("invalid merchant data")
                }
                PaymentError::InvalidAuth => {
                    HttpResponse::PaymentRequired().body("invalid authorization")
                }
                PaymentError::NoToken => HttpResponse::PaymentRequired().body("no token"),
                PaymentError::URIMalformed => HttpResponse::BadRequest().body("malformed URI"),
            },
        }
    }
}

#[derive(Debug)]
pub enum PaymentError {
    Content,
    Accept,
    Decode,
    Payload,
    NoMerchantDat,
    InvalidMerchantDat,
    InvalidAuth,
    NoToken,
    URIMalformed,
}

impl From<PaymentError> for ServerError {
    fn from(err: PaymentError) -> Self {
        ServerError::Payment(err)
    }
}

impl fmt::Display for PaymentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            PaymentError::Content => "invalid content-type",
            PaymentError::Accept => "not acceptable",
            PaymentError::Decode => "failed to decode body",
            PaymentError::Payload => "failed to receive payload",
            PaymentError::NoMerchantDat => "no merchant data",
            PaymentError::InvalidMerchantDat => "invalid merchant data",
            PaymentError::NoToken => "no token",
            PaymentError::InvalidAuth => "invalid authorization",
            PaymentError::URIMalformed => "malformed URI",
        };
        write!(f, "{}", printable)
    }
}
