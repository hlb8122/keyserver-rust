use std::pin::Pin;

use futures_core::{
    task::{Context, Poll},
    Future,
};
use futures_util::{future, FutureExt};
use hyper::{http::Method, Body, Error as HyperError, Request, Response};
use tower_service::Service;
use tower_util::ServiceExt;

use crate::{
    db::{errors::*, Database},
    SETTINGS,
};

const ROOT: &'static str = "";
const KEY_PATH: &'static str = "keys";

pub struct Keyserver<G> {
    getter: G,
    // putter: MetadataPutter,
}

impl<G> Keyserver<G> {
    pub fn new(getter: G) -> Self {
        // let putter = MetadataPutter::new(db);
        Keyserver { getter }
    }
}

async fn not_found() -> Result<Response<Body>, HyperError> {
    Ok(Response::builder().status(404).body(Body::empty()).unwrap())
}

async fn root_message() -> Result<Response<Body>, HyperError> {
    let root_message = Body::from(&SETTINGS.root_message[..]);
    let response = Response::builder().body(root_message).unwrap();
    Ok(response)
}

impl<G> Service<Request<Body>> for Keyserver<G>
where
    G: Service<String, Response = Vec<u8>, Error = GetError> + Clone + Send + 'static,
    G::Future: Send,
{
    type Response = Response<Body>;
    type Error = HyperError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        // Routing
        let mut path_split = request.uri().path().split_terminator('/');

        let first = if let Some(first) = path_split.next() {
            first
        } else {
            return Box::pin(not_found());
        };

        if let Some(second) = path_split.next() {
            if second != KEY_PATH {
                return Box::pin(not_found());
            }
        } else {
            if first == ROOT {
                return Box::pin(root_message());
            } else {
                return Box::pin(not_found());
            }
        }

        if let Some(key) = path_split.next() {
            if path_split.next().is_none() {
                // Match method
                match *request.method() {
                    Method::GET => {
                        let response =
                            self.getter
                                .clone()
                                .oneshot(key.to_string())
                                .map(|response| match response {
                                    Ok(metadata) => {
                                        Ok(Response::builder().body(Body::from(metadata)).unwrap())
                                    }
                                    Err(err) => Ok(err.into()),
                                });
                        Box::pin(response)
                    }
                    // Method::PUT => self.getter.clone().call(key.to_string()),
                    _ => Box::pin(not_found()),
                }
            } else {
                Box::pin(not_found())
            }
        } else {
            Box::pin(not_found())
        }
    }
}

pub struct MakeKeyserver<G, P> {
    getter: G,
    putter: P,
}

impl<G> MakeKeyserver<G, P> {
    pub fn new(getter: G, putter: P) -> Self {
        MakeKeyserver { getter, putter }
    }
}

impl<T, G, P> Service<T> for MakeKeyserver<G, P>
where
    G: Clone,
{
    type Response = Keyserver<G>;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let keyserver = Keyserver::new(self.db.clone(), self.get.clone());

        future::ok(keyserver)
    }
}
