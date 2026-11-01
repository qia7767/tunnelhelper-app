mod api;
mod config;
mod models;
mod service;
#[cfg(target_os = "linux")]
mod tray;

use api::ApiState;
use clap::Parser;
use config::{AuthMode, Cli, ServerConfig};
use service::CommandService;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Tunnel Helper failed to start: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = ServerConfig::from_cli(Cli::parse())?;
    let service = CommandService::new(&config)?;
    let address = SocketAddr::new(config.listen, config.port);
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("Cannot listen on {address}: {error}"))?;

    let access_token = config.access_token.clone().map(Arc::<str>::from);
    let app = api::router(ApiState {
        service: service.clone(),
        auth_mode: config.auth_mode,
        access_token,
    });

    println!("Tunnel Helper server is running");
    println!("Local UI: http://{}:{}", config.display_host(), config.port);
    println!("Data dir: {}", service.data_dir().display());
    match (&config.auth_mode, &config.access_token) {
        (AuthMode::Disabled, _) => println!("Access Token: disabled (loopback testing only)"),
        (AuthMode::Token, Some(token)) => println!("Access Token: {token}"),
        _ => {}
    }
    println!("Press Ctrl+C to stop the service and all managed tunnels.");

    #[cfg(target_os = "linux")]
    let (tray_shutdown, _tray_handle) = tray::start(config.display_host(), config.port).await;

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            #[cfg(target_os = "linux")]
            tokio::select! {
                _ = shutdown_signal() => {},
                _ = tray_shutdown => {},
            }
            #[cfg(not(target_os = "linux"))]
            shutdown_signal().await;
        })
        .await
        .map_err(|error| error.to_string())?;
    service.stop_all().await;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
