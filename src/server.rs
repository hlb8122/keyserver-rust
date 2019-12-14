use std::pin::Pin;

use futures_core::{
    task::{Context, Poll},
    Future,
};
use futures_util::future;
use hyper::{Body, Request, Response};
use tower_service::Service;

use crate::{
    db::{services::*, Database},
    SETTINGS,
};

const ROOT: &'static str = "/";
const KEY_PATH: &'static str = "/keys/";

pub struct Keyserver {
    getter: MetadataGetter,
    putter: MetadataPutter,
}

impl Keyserver {
    pub fn new(db: Database) -> Self {
        let getter = MetadataGetter::new(db.clone());
        let putter = MetadataPutter::new(db);
        Keyserver { getter, putter }
    }
}

impl Service<Request<Body>> for Keyserver {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        match request.uri().path() {
            ROOT => {
                let root_message = Body::from(&SETTINGS.root_message[..]);
                let response = Response::builder().body(root_message).unwrap();
                Box::pin(future::ok(response))
            }
            other => match &other[..KEY_PATH.len()] {
                KEY_PATH => unreachable!(),
                _ => unreachable!(),
            },
        }
    }
}

pub struct MakeKeyserver {
    db: Database,
}

impl MakeKeyserver {
    pub fn new(db: Database) -> Self {
        MakeKeyserver { db }
    }
}

impl<T> Service<T> for MakeKeyserver {
    type Response = Keyserver;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let keyserver = Keyserver::new(self.db.clone());

        future::ok(keyserver)
    }
}
