use std::time::{Duration, SystemTime, UNIX_EPOCH};

use actix_web::{
    http::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, PRAGMA},
    web, HttpMessage, HttpRequest, HttpResponse,
};
use futures::{
    future::{err, ok, Future},
    stream::Stream,
};
use prost::Message;

use crate::{
    crypto::address::Network,
    models::{Payment, PaymentAck, PaymentDetails},
};

use super::{errors::*, token::generate_token};

const SECRET: &[u8] = b"";
const PAYMENT_URL: &str = "/payment";
const VALID_DURATION: u64 = 30;
const NETWORK: Network = Network::Mainnet;

pub fn payment_handler(
    req: HttpRequest,
    payload: web::Payload,
) -> Box<Future<Item = HttpResponse, Error = ServerError>> {
    let headers = req.headers();
    if headers.get(ACCEPT) != Some(&HeaderValue::from_str("application/bitcoin-payment").unwrap()) {
        return Box::new(err(PaymentError::Accept.into()));
    }

    if headers.get(CONTENT_TYPE)
        != Some(&HeaderValue::from_str("application/bitcoin-paymentack").unwrap())
    {
        return Box::new(err(PaymentError::Content.into()));
    }

    // Read payment proto
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
            None => return err(PaymentError::NoMerchant.into()),
        };

        let token = generate_token(&merchant_data, SECRET);

        // Generate response
        ok(HttpResponse::Found()
            .header(AUTHORIZATION, format!("POP {}", hex::encode(token)))
            .header(PRAGMA, "no-cache")
            .body(raw_ack))
    });

    Box::new(response)
}

impl Into<String> for Network {
    fn into(self) -> String {
        match self {
            Network::Mainnet => "main".to_string(),
            Network::Testnet => "test".to_string(),
        }
    }
}

// let response = payment_ack.and_then(|ack| {
//     let merchant_data = match ack.payment.merchant_data {
//         Some(some) => some,
//         None => return err(PaymentError::NoMerchant),
//     };
//     let token = generate_token(&merchant_data, SECRET);
//     let current_time = SystemTime::now();
//     let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

//     let payment_invoice = PaymentDetails {
//         network: Some(NETWORK.into()),
//         time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
//         expires: Some(expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs()),
//         memo,
//         merchant_data: Some(merchant_data),
//         outputs: vec![],
//         payment_url: Some(PAYMENT_URL.to_string()),
//     };
//     ok(token)
// });
