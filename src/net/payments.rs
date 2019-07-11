use std::{
    str,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use actix_service::{Service, Transform};
use actix_web::{
    dev::{Body, ServiceRequest, ServiceResponse},
    http::{
        header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, LOCATION, PRAGMA},
        Method,
    },
    web, Error, HttpRequest, HttpResponse,
};
use futures::{
    future::{err, ok, Either, Future, FutureResult},
    stream::Stream,
    Poll,
};
use prost::Message;
use serde_derive::Deserialize;
use url::Url;

use crate::{
    bitcoin::Network,
    models::{Payment, PaymentAck, PaymentDetails, PaymentRequest},
};

use super::{errors::*, token::*};

const SECRET: &[u8] = b"deadbeef";
const PAYMENT_URL: &str = "/payment";
const VALID_DURATION: u64 = 30;
const NETWORK: Network = Network::Mainnet;

#[derive(Deserialize)]
pub struct TokenQuery {
    code: String,
}

///Payment handler
pub fn payment_handler(
    req: HttpRequest,
    payload: web::Payload,
) -> Box<Future<Item = HttpResponse, Error = ServerError>> {
    println!("Got here");
    // Check headers
    let headers = req.headers();
    if headers.get(ACCEPT) != Some(&HeaderValue::from_str("application/bitcoin-payment").unwrap()) {
        return Box::new(err(PaymentError::Accept.into()));
    }
    if headers.get(CONTENT_TYPE)
        != Some(&HeaderValue::from_str("application/bitcoin-paymentack").unwrap())
    {
        return Box::new(err(PaymentError::Content.into()));
    }

    // Read and parse payment proto
    let body_raw = payload.map_err(|_| PaymentError::Payload.into()).fold(
        web::BytesMut::new(),
        move |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, ServerError>(body)
        },
    );
    let payment = body_raw.and_then(|payment_raw| {
        Payment::decode(payment_raw).map_err(|_| PaymentError::Decode.into())
    });

    // TODO: Check outputs

    let memo = Some("Thanks for your custom!".to_string());
    let payment_ack = payment.map(|payment| PaymentAck { payment, memo });

    let response = payment_ack.and_then(|ack| {
        // Encode payment ack
        let mut raw_ack = Vec::with_capacity(ack.encoded_len());
        ack.encode(&mut raw_ack).unwrap();

        // Get merchant data
        let merchant_data = match ack.payment.merchant_data {
            Some(some) => some,
            None => return Err(PaymentError::NoMerchantDat.into()),
        };

        // Generate token
        let token = base64::encode(&generate_token(&merchant_data, SECRET));

        // Generate paymentredirect
        let mut redirect_url = Url::parse(
            str::from_utf8(&merchant_data).map_err(|_| PaymentError::InvalidMerchantDat)?,
        )
        .map_err(|_| PaymentError::InvalidMerchantDat)?;
        redirect_url.set_query(Some(&format!("code={}", token)));

        // Generate response
        Ok(HttpResponse::Found()
            .header(LOCATION, redirect_url.into_string())
            .header(AUTHORIZATION, format!("POP {}", token))
            .header(PRAGMA, "no-cache")
            .body(raw_ack))
    });

    Box::new(response)
}

/*
Payment middleware
*/

pub struct CheckPayment;

impl<S> Transform<S> for CheckPayment
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckPaymentMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CheckPaymentMiddleware { service })
    }
}
pub struct CheckPaymentMiddleware<S> {
    service: S,
}

impl<S> Service for CheckPaymentMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type Future = Either<S::Future, FutureResult<Self::Response, Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        // Only pay for put
        match *req.method() {
            Method::GET => return Either::A(self.service.call(req)),
            Method::PUT => (),
            _ => unreachable!(),
        }

        // Grab token query from authorization header then query string
        let token_str: String = match req.headers().get(AUTHORIZATION) {
            Some(some) => match some.to_str() {
                Ok(auth_str) => {
                    if auth_str.len() >= 4 && &auth_str[0..4] == "POP " {
                        auth_str[4..].to_string()
                    } else {
                        return Either::B(err(
                            ServerError::Payment(PaymentError::InvalidAuth).into()
                        ));
                    }
                }
                Err(_) => {
                    return Either::B(err(ServerError::Payment(PaymentError::InvalidAuth).into()))
                }
            },
            None => match web::Query::<TokenQuery>::from_query(req.query_string()) {
                Ok(query) => query.code.clone(), // TODO: Remove this copy
                Err(_) => {
                    return Either::B(err(ServerError::Payment(PaymentError::NoToken).into()))
                }
            },
        };

        // Decode token
        let token = match base64::decode(&token_str) {
            Ok(some) => some,
            Err(_) => {
                return Either::B(err(ServerError::Payment(PaymentError::InvalidAuth).into()))
            }
        };

        // Validate
        let path_raw = req.path().as_bytes();
        if !validate_token(path_raw, &token, SECRET) {
            return Either::B(err(ServerError::Payment(PaymentError::InvalidAuth).into()));
        }

        // Valid interval
        let current_time = SystemTime::now();
        let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

        // Generate payment details
        let payment_details = PaymentDetails {
            network: Some(NETWORK.into()),
            time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            expires: Some(expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs()),
            memo: None,
            merchant_data: Some(path_raw.to_vec()),
            outputs: vec![],
            payment_url: Some(PAYMENT_URL.to_string()),
        };
        let mut serialized_payment_details = Vec::with_capacity(payment_details.encoded_len());
        payment_details
            .encode(&mut serialized_payment_details)
            .unwrap();

        // Generate payment invoice
        let pki_type = Some("none".to_string());
        let payment_invoice = PaymentRequest {
            pki_type,
            pki_data: None,
            payment_details_version: Some(1),
            serialized_payment_details,
            signature: None,
        };
        let mut payment_invoice_raw = Vec::with_capacity(payment_invoice.encoded_len());
        payment_invoice.encode(&mut payment_invoice_raw).unwrap();

        // Respond
        Either::B(ok(req.into_response(
            HttpResponse::PaymentRequired().body(payment_invoice_raw),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bitcoin::Network,
        crypto::{ecdsa::Secp256k1PublicKey, *},
        models::*,
        net::{*, tests::generate_address_metadata},
        db::KeyDB,
    };
    use actix_service::Service;
    use actix_web::{http::StatusCode, test, web, App};
    use secp256k1::{rand, Secp256k1};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_put_payment() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_ok").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(State(key_db))
                .wrap(CheckPayment) // Apply payment check to put key
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        let (address_base58, metadata_raw) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(metadata_raw)
            .to_request();
    }
}