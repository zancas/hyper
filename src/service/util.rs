use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt;
use std::marker::PhantomData;

use futures_util::future;

use crate::body::HttpBody;
use crate::common::{task, Future, Poll};
use crate::{Request, Response};

/// Create a `Service` from a function.
///
/// # Example
///
/// ```
/// use hyper::{Body, Request, Response, Version};
/// use hyper::service::service_fn;
///
/// let service = service_fn(|req: Request<Body>| async move {
///     if req.version() == Version::HTTP_11 {
///         Ok(Response::new(Body::from("Hello World")))
///     } else {
///         // Note: it's usually better to return a Response
///         // with an appropriate StatusCode instead of an Err.
///         Err("not HTTP/1.1, abort connection")
///     }
/// });
/// ```
pub fn service_fn<F, R, S>(f: F) -> ServiceFn<F, R>
where
    F: FnMut(Request<R>) -> S,
    S: Future,
{
    ServiceFn {
        f,
        _req: PhantomData,
    }
}

/// Create a `Service` that responds by cloning the value.
pub fn shared<T>(value: T) -> Shared<T> {
    Shared { value }
}

// Service returned by [`service_fn`]
pub struct ServiceFn<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}

#[derive(Debug, Clone)]
pub struct Shared<T> {
    value: T,
}

// ===== impl ServiceFn =====

impl<F, ReqBody, Ret, ResBody, E> tower_service::Service<crate::Request<ReqBody>>
    for ServiceFn<F, ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Ret,
    ReqBody: HttpBody,
    Ret: Future<Output = Result<Response<ResBody>, E>>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ResBody: HttpBody,
{
    type Response = crate::Response<ResBody>;
    type Error = E;
    type Future = Ret;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        (self.f)(req)
    }
}

impl<F, R> fmt::Debug for ServiceFn<F, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("impl Service").finish()
    }
}

impl<F, R> Clone for ServiceFn<F, R>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        ServiceFn {
            f: self.f.clone(),
            _req: PhantomData,
        }
    }
}

impl<F, R> Copy for ServiceFn<F, R> where F: Copy {}

// ===== impl Shared =====

impl<T, Req> tower_service::Service<Req> for Shared<T>
where
    T: Clone,
{
    type Response = T;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Req) -> Self::Future {
        future::ok(self.value.clone())
    }
}
