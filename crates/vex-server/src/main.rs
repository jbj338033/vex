mod config;
mod db;
mod middleware;
mod routes;
mod services;

use std::net::SocketAddr;

use anyhow::Result;

use crate::services::{proxy, tls};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vex_server=info".into()),
        )
        .init();

    let config = config::Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;

    let (tls_acceptor, challenge_store, cert_store) = if config.tls.enabled {
        let (acceptor, challenges, certs) = tls::init(&config.tls, &config.domain).await?;
        (Some(acceptor), Some(challenges), Some(certs))
    } else {
        (None, None, None)
    };

    let state = routes::AppState::new(pool, config, challenge_store.clone(), cert_store);

    let api_router = routes::router(state.clone());

    if let Some(challenges) = challenge_store {
        tokio::spawn(proxy::serve_http_challenge(challenges));
    }

    let proxy_addr: SocketAddr = state.config.proxy_listen_addr.parse()?;
    let route_table = state.route_table.clone();
    let listen_addr = state.config.listen_addr.clone();

    let proxy_task = tokio::spawn(proxy::serve(proxy_addr, route_table, tls_acceptor));
    let api_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&listen_addr)
            .await
            .expect("failed to bind api listener");
        tracing::info!("api server listening on {listen_addr}");
        axum::serve(listener, api_router)
            .await
            .expect("api server error");
    });

    tokio::select! {
        r = proxy_task => r?,
        r = api_task => r?,
    }

    Ok(())
}
