use actix_service::{Service, Transform};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use futures::{
    future::{ok, FutureResult},
    Future, Poll,
};

pub struct PaymentEnforcer;
