use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use dashmap::DashMap;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::services::tls::ChallengeStore;

type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

#[derive(Clone)]
pub struct RouteTable {
    routes: Arc<DashMap<String, RouteTarget>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RouteTarget {
    pub host_port: u16,
    pub container_id: String,
}

impl RouteTable {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, app_name: String, target: RouteTarget) {
        self.routes.insert(app_name, target);
    }

    pub fn remove(&self, app_name: &str) {
        self.routes.remove(app_name);
    }

    pub fn get(&self, app_name: &str) -> Option<RouteTarget> {
        self.routes.get(app_name).as_deref().cloned()
    }
}

pub async fn serve(
    addr: SocketAddr,
    route_table: RouteTable,
    tls_acceptor: Option<TlsAcceptor>,
    api_port: Option<u16>,
) {
    let listener = TcpListener::bind(addr)
        .await
        .expect("failed to bind proxy listener");

    let scheme = if tls_acceptor.is_some() {
        "https"
    } else {
        "http"
    };
    tracing::info!("proxy server listening on {scheme}://{addr}");

    let client: Client<_, Incoming> = Client::builder(TokioExecutor::new()).build_http();

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("proxy accept error: {e}");
                continue;
            }
        };

        let route_table = route_table.clone();
        let client = client.clone();
        let tls = tls_acceptor.clone();

        tokio::spawn(async move {
            if let Some(acceptor) = tls {
                let tls_stream = match acceptor.accept(stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::debug!("tls handshake failed: {e}");
                        return;
                    }
                };
                serve_connection(
                    hyper_util::rt::TokioIo::new(tls_stream),
                    route_table,
                    client,
                    api_port,
                )
                .await;
            } else {
                serve_connection(
                    hyper_util::rt::TokioIo::new(stream),
                    route_table,
                    client,
                    api_port,
                )
                .await;
            }
        });
    }
}

async fn serve_connection<I>(
    io: I,
    route_table: RouteTable,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Incoming>,
    api_port: Option<u16>,
) where
    I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    let svc = service_fn(move |req| {
        let rt = route_table.clone();
        let c = client.clone();
        async move { handle(req, rt, c, api_port).await }
    });

    if let Err(e) = http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(io, svc)
        .with_upgrades()
        .await
    {
        tracing::debug!("proxy connection error: {e}");
    }
}

async fn handle(
    req: Request<Incoming>,
    route_table: RouteTable,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Incoming>,
    api_port: Option<u16>,
) -> Result<Response<BoxBody>, Infallible> {
    let app_name = match extract_app_name(req.headers()) {
        Some(name) => name,
        None => return Ok(not_found()),
    };

    let upstream_port = if app_name == "api" {
        match api_port {
            Some(port) => port,
            None => return Ok(not_found()),
        }
    } else {
        match route_table.get(&app_name) {
            Some(t) => t.host_port,
            None => return Ok(not_found()),
        }
    };

    let (mut parts, body) = req.into_parts();

    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri = format!("http://127.0.0.1:{upstream_port}{path_and_query}");
    parts.uri = uri.parse().expect("constructed uri must be valid");
    parts.headers.remove(hyper::header::HOST);

    let upstream_req = Request::from_parts(parts, body);

    match client.request(upstream_req).await {
        Ok(resp) => Ok(resp.map(|b| b.boxed())),
        Err(e) => {
            tracing::error!("upstream request failed: {e}");
            Ok(bad_gateway())
        }
    }
}

fn extract_app_name(headers: &hyper::HeaderMap) -> Option<String> {
    let host = headers.get(hyper::header::HOST)?.to_str().ok()?;
    let without_port = host.split(':').next()?;
    let subdomain = without_port.split('.').next()?;
    if subdomain == without_port {
        return None;
    }
    Some(subdomain.to_string())
}

fn bad_gateway() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Empty::new().map_err(|never| match never {}).boxed())
        .expect("building 502 response must not fail")
}

fn not_found() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Empty::new().map_err(|never| match never {}).boxed())
        .expect("building 404 response must not fail")
}

pub async fn serve_http_challenge(challenge_store: ChallengeStore) {
    let addr: SocketAddr = "0.0.0.0:80".parse().unwrap();
    let listener = TcpListener::bind(addr)
        .await
        .expect("failed to bind http challenge listener on :80");

    tracing::info!("http challenge listener on {addr}");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("http challenge accept error: {e}");
                continue;
            }
        };

        let store = challenge_store.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| {
                let s = store.clone();
                async move { handle_challenge(req, s) }
            });

            if let Err(e) = http1::Builder::new()
                .serve_connection(hyper_util::rt::TokioIo::new(stream), svc)
                .await
            {
                tracing::debug!("http challenge connection error: {e}");
            }
        });
    }
}

fn handle_challenge(
    req: Request<Incoming>,
    challenge_store: ChallengeStore,
) -> Result<Response<BoxBody>, Infallible> {
    let path = req.uri().path();

    if let Some(token) = path.strip_prefix("/.well-known/acme-challenge/") {
        if let Some(key_auth) = challenge_store.get(token) {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/octet-stream")
                .body(
                    Full::new(Bytes::from(key_auth))
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .expect("building challenge response must not fail"));
        }
        return Ok(not_found());
    }

    let host = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let location = format!("https://{host}{path}");

    Ok(Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header("location", location)
        .body(Empty::new().map_err(|never| match never {}).boxed())
        .expect("building redirect response must not fail"))
}
