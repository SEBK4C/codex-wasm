//! codex-wasm: on wasm32-wasip1 every request goes through the browser tab's `fetch`
//! (codex-wasm-host) instead of hyper. A tab has no sockets; `fetch` does TLS, HTTP/2,
//! redirects and decompression itself. Client settings that only make sense for a socket
//! (proxies, TLS roots, pools, connect timeouts) have no effect here; the total timeout is
//! kept.

use super::body::Body;
use super::response::Response;
use crate::response::ResponseUrl;
use bytes::Bytes;
use codex_wasm_host::Http;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use url::Url;

fn request_error(e: impl Into<crate::error::BoxError>, url: &Url) -> crate::Error {
    crate::error::request(e).with_url(url.clone())
}

pub(super) async fn execute(
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Body>,
    total_timeout: Option<Duration>,
) -> crate::Result<Response> {
    let run = run(method, url.clone(), headers, body);
    match total_timeout {
        Some(limit) => match tokio::time::timeout(limit, run).await {
            Ok(result) => result,
            Err(_) => Err(request_error(crate::error::TimedOut, &url)),
        },
        None => run.await,
    }
}

async fn run(method: Method, url: Url, headers: HeaderMap, body: Option<Body>) -> crate::Result<Response> {
    let body: Bytes = match body {
        None => Bytes::new(),
        Some(body) => match body.as_bytes() {
            Some(bytes) => Bytes::copy_from_slice(bytes),
            None => {
                use http_body_util::BodyExt;
                body.collect().await.map_err(|e| request_error(e, &url))?.to_bytes()
            }
        },
    };

    let header_list: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_owned(), v.to_owned())))
        .collect();
    let spec = serde_json::json!({
        "method": method.as_str(),
        "url": url.as_str(),
        "headers": header_list,
    });
    let spec = serde_json::to_vec(&spec).map_err(|e| request_error(e, &url))?;
    let http = Http::start(&spec, &body).map_err(|e| request_error(e, &url))?;

    let head = poll_fn(|cx| match http.poll_head(cx.waker()) {
        Some(head) => Poll::Ready(head),
        None => Poll::Pending,
    })
    .await;
    let head: serde_json::Value = serde_json::from_slice(&head).map_err(|e| request_error(e, &url))?;
    if let Some(error) = head.get("error").and_then(|e| e.as_str()) {
        return Err(request_error(io::Error::new(io::ErrorKind::ConnectionRefused, error.to_owned()), &url));
    }
    let status = head
        .get("status")
        .and_then(|s| s.as_u64())
        .and_then(|s| StatusCode::from_u16(s as u16).ok())
        .ok_or_else(|| request_error(io::Error::other("the tab answered without a status"), &url))?;
    let final_url = head
        .get("url")
        .and_then(|u| u.as_str())
        .and_then(|u| Url::parse(u).ok())
        .unwrap_or_else(|| url.clone());

    let mut builder = http::Response::builder().status(status);
    if let Some(list) = head.get("headers").and_then(|h| h.as_array()) {
        let map = builder.headers_mut().expect("a fresh builder has headers");
        for pair in list {
            let (Some(k), Some(v)) = (pair.get(0).and_then(|k| k.as_str()), pair.get(1).and_then(|v| v.as_str())) else {
                continue;
            };
            if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                map.append(k, v);
            }
        }
    }
    let response = builder
        .extension(ResponseUrl(final_url))
        .body(Body::wrap(HostBody { http, buf: vec![0u8; 64 * 1024], done: false }))
        .map_err(|e| request_error(e, &url))?;
    Ok(Response::from(response))
}

struct HostBody {
    http: Http,
    buf: Vec<u8>,
    done: bool,
}

impl http_body::Body for HostBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, io::Error>>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        match this.http.read(&mut this.buf, cx.waker()) {
            Ok(None) => Poll::Pending,
            Ok(Some(0)) => {
                this.done = true;
                Poll::Ready(None)
            }
            Ok(Some(n)) => Poll::Ready(Some(Ok(http_body::Frame::data(Bytes::copy_from_slice(&this.buf[..n]))))),
            Err(e) => {
                this.done = true;
                Poll::Ready(Some(Err(e)))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.done
    }
}
