use std::fmt;

use actix_web::{error, HttpResponse};
use bitcoin::consensus::encode::Error as TxDeserializeError;
use bitcoincash_addr::{base58, cashaddr};
use prost::DecodeError;
use rocksdb::Error as RocksError;

use crate::crypto::errors::CryptoError;

#[derive(Debug)]
pub enum ValidationError {
    KeyType,
    Preimage,
    Outdated,
    ExpiredTTL,
    Crypto(CryptoError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ValidationError::KeyType => "bad key type",
            ValidationError::Preimage => "digest mismatch",
            ValidationError::Outdated => "metadata is outdated",
            ValidationError::ExpiredTTL => "expired TTL",
            ValidationError::Crypto(err) => return err.fmt(f),
        };
        write!(f, "{}", printable)
    }
}

impl Into<ValidationError> for CryptoError {
    fn into(self) -> ValidationError {
        ValidationError::Crypto(self)
    }
}

#[derive(Debug)]
pub enum ServerError {
    DB(RocksError),
    Validation(ValidationError),
    Crypto(CryptoError),
    NotFound,
    MetadataDecode,
    PayloadDecode,
    UnsupportedSigScheme,
    Payment(PaymentError),
    Address(cashaddr::DecodingError, base58::DecodingError),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ServerError::DB(err) => return err.fmt(f),
            ServerError::Crypto(err) => return err.fmt(f),
            ServerError::NotFound => "not found",
            ServerError::MetadataDecode => "metadata decoding error",
            ServerError::PayloadDecode => "payload decoding error",
            ServerError::UnsupportedSigScheme => "signature scheme not supported",
            ServerError::Payment(err) => return err.fmt(f),
            ServerError::Validation(err) => return err.fmt(f),
            ServerError::Address(cash_err, base58_err) => {
                return write!(f, "{}, {}", cash_err, base58_err)
            }
        };
        write!(f, "{}", printable)
    }
}

impl From<(cashaddr::DecodingError, base58::DecodingError)> for ServerError {
    fn from((cash_err, base58_err): (cashaddr::DecodingError, base58::DecodingError)) -> Self {
        ServerError::Address(cash_err, base58_err)
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

impl From<ValidationError> for ServerError {
    fn from(err: ValidationError) -> ServerError {
        ServerError::Validation(err)
    }
}

impl error::ResponseError for CryptoError {
    fn error_response(&self) -> HttpResponse {
        match self {
            CryptoError::PubkeyDeserialization => HttpResponse::BadRequest(),
            CryptoError::SigDeserialization => HttpResponse::BadRequest(),
            CryptoError::Verification => HttpResponse::BadRequest(),
        }
        .body(self.to_string())
    }
}

impl error::ResponseError for ValidationError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ValidationError::Crypto(err_inner) => return err_inner.error_response(),
            ValidationError::KeyType => HttpResponse::BadRequest(),
            ValidationError::Preimage => HttpResponse::BadRequest(),
            ValidationError::Outdated => HttpResponse::BadRequest(),
            ValidationError::ExpiredTTL => HttpResponse::BadRequest(),
        }
        .body(self.to_string())
    }
}

impl error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServerError::Validation(err) => err.error_response(),
            // Do not yield sensitive information to clients
            ServerError::DB(_) => HttpResponse::InternalServerError().body("internal db error"),
            ServerError::NotFound => HttpResponse::NotFound().body(self.to_string()),
            ServerError::MetadataDecode => HttpResponse::BadRequest().body(self.to_string()),
            ServerError::PayloadDecode => HttpResponse::BadRequest().body(self.to_string()),
            ServerError::UnsupportedSigScheme => HttpResponse::BadRequest().body(self.to_string()),
            ServerError::Crypto(err) => err.error_response(),
            ServerError::Payment(err) => err.error_response(),
            ServerError::Address(cash_err, base58_err) => {
                HttpResponse::BadRequest().body(self.to_string())
            }
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
    NoTx,
    TxDeserialize(TxDeserializeError),
    InvalidOutputs,
    InvalidTx,
    MismatchedNetwork,
    AddrFetchFailed,
}

impl From<PaymentError> for ServerError {
    fn from(err: PaymentError) -> Self {
        ServerError::Payment(err)
    }
}

impl From<TxDeserializeError> for PaymentError {
    fn from(err: TxDeserializeError) -> PaymentError {
        PaymentError::TxDeserialize(err)
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
            PaymentError::NoTx => "no payment tx",
            PaymentError::TxDeserialize(_) => "payment tx malformed",
            PaymentError::InvalidOutputs => "invalid outputs",
            PaymentError::InvalidTx => "invalid tx",
            PaymentError::AddrFetchFailed => "failed to fetch address",
            PaymentError::MismatchedNetwork => "address mismatched with node network",
        };
        write!(f, "{}", printable)
    }
}

impl error::ResponseError for PaymentError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PaymentError::Accept => HttpResponse::NotAcceptable(),
            PaymentError::Content => HttpResponse::UnsupportedMediaType(),
            PaymentError::NoMerchantDat => HttpResponse::BadRequest(),
            PaymentError::Payload => HttpResponse::BadRequest(),
            PaymentError::Decode => HttpResponse::BadRequest(),
            PaymentError::InvalidMerchantDat => HttpResponse::BadRequest(),
            PaymentError::InvalidAuth => HttpResponse::PaymentRequired(),
            PaymentError::NoToken => HttpResponse::PaymentRequired(),
            PaymentError::URIMalformed => HttpResponse::BadRequest(),
            PaymentError::NoTx => HttpResponse::BadRequest(),
            PaymentError::TxDeserialize(_) => HttpResponse::BadRequest(),
            PaymentError::InvalidOutputs => HttpResponse::BadRequest(),
            PaymentError::InvalidTx => HttpResponse::BadRequest(),
            PaymentError::MismatchedNetwork => HttpResponse::BadRequest(),
            PaymentError::AddrFetchFailed => HttpResponse::InternalServerError(),
        }
        .body(self.to_string())
    }
}
