use std::fmt;

use actix_web::{error, HttpResponse};

use crate::crypto::errors::CryptoError;

#[derive(Debug)]
pub enum ServerError {
    DB,
    Crypto(CryptoError),
    NotFound,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            ServerError::DB => "database error",
            ServerError::Crypto(err) => return err.fmt(f),
            ServerError::NotFound => "not found",
        };
        write!(f, "{}", printable)
    }
}

impl From<CryptoError> for ServerError {
    fn from(err: CryptoError) -> Self {
        ServerError::Crypto(err)
    }
}

impl error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServerError::DB => {
                HttpResponse::InternalServerError().body("database read failure")
            }
            ServerError::NotFound => HttpResponse::NotFound().body("missing key address"),
            ServerError::Crypto(err) => match err {
                CryptoError::Deserialization => HttpResponse::BadRequest().body("invalid address"),
                CryptoError::NonHexAddress => HttpResponse::BadRequest().body("non-hex address"),
                CryptoError::Verification => HttpResponse::BadRequest().body("validation failed"),
            },
        }
    }
}
