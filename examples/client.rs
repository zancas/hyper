#![warn(rust_2018_idioms)]
use core::{
    future::Future,
    ops::Deref,
    task::{Context, Poll},
};
use futures_executor::block_on;
use futures_util::future::{ready, Ready};
use serde::{Deserialize, Serialize};
use std::{boxed::Box, env, error::Error as StdError, pin::Pin};

use hyper::{
    body::HttpBody as _,
    client::connect::{Connected, Connection},
    Body, Client, Request, Uri,
};
use tokio::io::{self, AsyncRead, AsyncWrite, AsyncWriteExt as _, Error};
use tower_service::Service;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request as WebRequest, Response};

type StdErr = dyn StdError + Send + Sync + 'static;

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Some simple CLI args requirements...
    let url = "http://127.0.0.1:3030";
    /*match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return Ok(());
        }
    };*/

    // HTTPS requires picking a TLS implementation, so give a better
    // warning if the user tries to request an 'https' URL.
    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_str() != Some("http") {
        println!("This example only works with 'http' URLs.");
        return Ok(());
    }

    match fetch_url(url).await {
        Ok(_) => Ok(()),
        Err(e) => Err(JsValue::from(e.to_string())),
    }
}

async fn fetch_url(url: hyper::Uri) -> Result<(), Box<StdErr>> {
    use std::time::Duration;

    #[derive(Clone)]
    struct ConnectImplConformer(Vec<u8>);

    impl AsyncRead for ConnectImplConformer {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut Box::new(self.0.as_slice())).poll_read(_cx, buf)
        }
    }

    impl AsyncWrite for ConnectImplConformer {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, Error>> {
            Pin::new(&mut Box::new(self.0.clone())).poll_write(cx, buf)
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            cx: &mut Context,
        ) -> Poll<Result<(), Error>> {
            Pin::new(&mut Box::new(self.0.clone())).poll_flush(cx)
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            cx: &mut Context,
        ) -> Poll<Result<(), Error>> {
            Pin::new(&mut Box::new(self.0.clone())).poll_shutdown(cx)
        }
    }

    impl Connection for ConnectImplConformer {
        fn connected(&self) -> Connected {
            Connected::new()
        }
    }

    //    impl Unpin for LocalFuture {}

    //    struct LocalFuture(Result<ConnectImplConformer, Box<StdErr>>);

    //    impl Future for LocalFuture {
    //        type Output = Result<ConnectImplConformer, Box<StdErr>>;
    //
    //        fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    //            Poll::Ready(match &self.0 {
    //                Ok(x) => Ok(x.clone()),
    //                Err(e) => Err(<Box<StdErr>>::from(e.to_string())),
    //            })
    //        }
    //    }

    #[derive(Clone)]
    struct LocalConnect;

    impl Service<Uri> for LocalConnect {
        type Response = ConnectImplConformer;
        type Error = Box<StdErr>;
        type Future = Ready<Result<ConnectImplConformer, Box<StdErr>>>;

        fn poll_ready(
            &mut self,
            cx: &mut Context,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, dst: Uri) -> Self::Future {
            ready(block_on(async {
                match JsFuture::from(
                    web_sys::window().unwrap().fetch_with_request(
                        &WebRequest::new_with_str(dst.path()).ok().unwrap(),
                    ),
                )
                .await
                {
                    Ok(m) => match m.dyn_into::<Response>().unwrap().status() {
                        200..=299 => Ok(ConnectImplConformer(Vec::new())),
                        e @ _ => Err(<Box<StdErr>>::from(format!(
                            "Error code: {}",
                            e
                        ))),
                    },
                    Err(e) => Err(<Box<StdErr>>::from(e.as_string().unwrap())),
                }
            }))
        }
    }

    //let response = match get_response(dst).await {
    //    Ok(res) => res,
    //    Err(e) => return LocalFuture { Err(e) },
    //}

    let client = Client::builder()
        .pool_idle_timeout(Duration::from_secs(30))
        .http2_only(true)
        .build::<_, Body>(LocalConnect);

    let mut res = client.get(url).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    // Stream the body, writing each chunk to stdout as we get it
    // (instead of buffering and printing at the end).
    while let Some(next) = res.data().await {
        let chunk = next?;
        io::stdout().write_all(&chunk).await?;
    }

    println!("\n\nDone!");

    Ok(())
}

async fn get_response(url: Uri) -> Result<JsValue, JsValue> {
    let request = WebRequest::new_with_str(url.path())?;
    let response =
        JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request))
            .await?
            .dyn_into::<Response>()
            .unwrap();
    JsFuture::from(response.json()?).await
}
