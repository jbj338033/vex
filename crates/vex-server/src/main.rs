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

    let state = routes::AppState::new(pool, config, challenge_store.clone(), cert_store.clone());

    let api_router = routes::router(state.clone());

    if let Some(ref challenges) = challenge_store {
        tokio::spawn(proxy::serve_http_challenge(challenges.clone()));
    }

    let api_listen_addr: SocketAddr = state.config.listen_addr.parse()?;
    let api_port = if state.config.tls.enabled {
        let api_fqdn = format!("api.{}", state.config.domain);
        if let (Some(challenges), Some(certs)) = (&challenge_store, &cert_store)
            && let Err(e) =
                tls::provision_app(&api_fqdn, &state.config.tls, challenges, certs).await
        {
            tracing::error!("failed to provision api cert for {api_fqdn}: {e}");
        }
        Some(api_listen_addr.port())
    } else {
        None
    };

    let proxy_addr: SocketAddr = state.config.proxy_listen_addr.parse()?;
    let route_table = state.route_table.clone();

    let proxy_task = tokio::spawn(proxy::serve(
        proxy_addr,
        route_table,
        tls_acceptor,
        api_port,
    ));
    let api_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(api_listen_addr)
            .await
            .expect("failed to bind api listener");
        tracing::info!("api server listening on {api_listen_addr}");
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
