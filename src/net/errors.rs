use std::fmt;

use bitcoin::consensus::encode::Error as TxDeserializeError;
use bitcoincash_addr::AddressError;
use prost::DecodeError;
use rocksdb::Error as RocksError;

use crate::crypto::errors::CryptoError;

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
    Address(AddressError),
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
            ServerError::Address(err) => return err.fmt(f),
        };
        write!(f, "{}", printable)
    }
}

impl From<AddressError> for ServerError {
    fn from(err: AddressError) -> Self {
        ServerError::Address(err)
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
