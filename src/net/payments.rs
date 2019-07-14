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
    web, Error, HttpRequest, HttpResponse, ResponseError,
};
use futures::{
    future::{err, ok, Either, Future, FutureResult},
    stream::Stream,
    Poll,
};
use prost::Message;
use serde_derive::Deserialize;
use url::Url;

use crate::{bitcoin::Network, models::*, SETTINGS};

use super::{errors::*, token::*};

const PAYMENT_URL: &str = "/payments";
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
        let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
        let token = base64::encode_config(&generate_token(&merchant_data, SETTINGS.secret.as_bytes()), url_safe_config);

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
            Method::PUT => (),
            _ => return Either::A(self.service.call(req)),
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
                // Found no token
                Err(_) => {
                    // Valid interval
                    let current_time = SystemTime::now();
                    let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

                    // Generate payment details
                    let uri = req.uri();
                    let url = match (uri.scheme_str(), uri.authority_part()) {
                        (Some(scheme), Some(authority)) => {
                            format!("{}://{}{}", scheme, authority, uri.path())
                        }
                        (_, _) => {
                            return Either::B(err(
                                ServerError::Payment(PaymentError::URIMalformed).into()
                            ))
                        }
                    };
                    let payment_details = PaymentDetails {
                        network: Some(NETWORK.into()),
                        time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        expires: Some(expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs()),
                        memo: None,
                        merchant_data: Some(url.as_bytes().to_vec()),
                        outputs: vec![],
                        payment_url: Some(PAYMENT_URL.to_string()),
                    };
                    let mut serialized_payment_details =
                        Vec::with_capacity(payment_details.encoded_len());
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
                    return Either::B(ok(req.into_response(
                        HttpResponse::PaymentRequired().body(payment_invoice_raw),
                    )));
                }
            },
        };

        // Decode token
        let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
        let token = match base64::decode_config(&token_str, url_safe_config) {
            Ok(some) => some,
            Err(_) => {
                return Either::B(ok(req.into_response(
                    ServerError::Payment(PaymentError::InvalidAuth).error_response(),
                )))
            }
        };

        // Validate
        let uri = req.uri();
        let url = match (uri.scheme_str(), uri.authority_part()) {
            (Some(scheme), Some(authority)) => format!("{}://{}{}", scheme, authority, uri.path()),
            (_, _) => {
                return Either::B(ok(req.into_response(
                    ServerError::Payment(PaymentError::URIMalformed).error_response(),
                )))
            }
        };
        if !validate_token(url.as_bytes(), &token, SETTINGS.secret.as_bytes()) {
            Either::B(ok(req.into_response(
                ServerError::Payment(PaymentError::InvalidAuth).error_response(),
            )))
        } else {
            Either::A(self.service.call(req))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::KeyDB,
        models::PaymentRequest,
        net::{tests::generate_address_metadata, *},
    };
    use actix_web::{http::StatusCode, test, web, App};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_put_no_token() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/no_token").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(State(key_db))
                .wrap(CheckPayment) // Apply payment check to put key
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_path = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_path)
            .set_payload(metadata_raw)
            .to_request();
        let mut resp = test::call_service(&mut app, req);

        // Check status
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);

        // Get invoice
        let body_vec = resp.take_body().collect().wait().unwrap();
        let invoice_raw = body_vec.get(0).unwrap();
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();

        // Check invoice is valid
        let payment_details = PaymentDetails::decode(invoice.serialized_payment_details).unwrap();
        assert_eq!(payment_details.network.unwrap(), "main".to_string());
        assert!(payment_details.expires.unwrap() > payment_details.time);
        assert!(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                >= payment_details.time
        );
        assert_eq!(
            payment_details.payment_url.unwrap(),
            "/payments".to_string()
        );
        assert_eq!(payment_details.merchant_data.unwrap(), key_path.as_bytes())
    }

    #[test]
    fn test_put_payment() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/payment").unwrap();
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("/keys").service(
                        web::resource("/{addr}")
                            .data(State(key_db))
                            .wrap(CheckPayment) // Apply payment check to put key
                            .route(web::put().to_async(put_key)),
                    ),
                )
                .service(web::resource("/payments").route(web::post().to_async(payment_handler))),
        );

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_url = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_url)
            .set_payload(metadata_raw)
            .to_request();
        let mut resp = test::call_service(&mut app, req);

        // Create payment
        let body_vec = resp.take_body().collect().wait().unwrap();
        let invoice_raw = body_vec.get(0).unwrap();
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();
        let payment_details = PaymentDetails::decode(invoice.serialized_payment_details).unwrap();
        let payment = Payment {
            merchant_data: payment_details.merchant_data,
            memo: None,
            refund_to: vec![],
            transactions: vec![],
        };
        let mut payment_raw = Vec::with_capacity(payment.encoded_len());
        payment.encode(&mut payment_raw).unwrap();

        // Check accept header is enforced
        let payment_path = payment_details.payment_url.unwrap();
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);

        // Check content-type header is enforced
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .header(ACCEPT, "application/bitcoin-payment")
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        // Check payment ack and a token
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .header(ACCEPT, "application/bitcoin-payment")
            .header(CONTENT_TYPE, "application/bitcoin-paymentack")
            .to_request();
        let mut resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::FOUND);
        let body_vec = resp.take_body().collect().wait().unwrap();
        let payment_ack_raw = body_vec.get(0).unwrap();
        let payment_ack = PaymentAck::decode(payment_ack_raw).unwrap();
        assert_eq!(payment, payment_ack.payment);

        // Check token
        let auth = resp.headers().get(AUTHORIZATION).unwrap();
        let loc = Url::parse(resp.headers().get(LOCATION).unwrap().to_str().unwrap()).unwrap();
        let loc_noquery = format!(
            "{}://{}:{}{}",
            loc.scheme(),
            loc.host().unwrap(),
            loc.port().unwrap(),
            loc.path()
        );
        assert_eq!(&loc_noquery, key_url);
        let pair = loc.query_pairs().next().unwrap();
        assert_eq!("code", pair.0);
        assert_eq!(&format!("POP {}", pair.1), auth.to_str().unwrap());

        // TODO: More detail here
        // Check token works with code
        let req = test::TestRequest::put().uri(loc.as_str()).to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);

        // TODO: More detail here
        // Check token works with POP token
        let req = test::TestRequest::put()
            .uri(loc.as_str())
            .header(AUTHORIZATION, auth.to_str().unwrap())
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    }
}
