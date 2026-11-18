#![cfg_attr(windows, windows_subsystem = "windows")]

mod api;
mod config;
mod models;
mod service;
#[cfg(target_os = "linux")]
mod tray;
#[cfg(windows)]
mod windows_tray;

use api::ApiState;
use clap::Parser;
use config::{AuthMode, Cli, ServerConfig};
use service::CommandService;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    let console_available = windows_tray::attach_parent_console();
    #[cfg(not(windows))]
    let console_available = true;
    if let Err(error) = run(console_available).await {
        eprintln!("Tunnel Helper failed to start: {error}");
        #[cfg(windows)]
        if !console_available { windows_tray::show_start_error(&error); }
        std::process::exit(1);
    }
}

async fn run(_console_available: bool) -> Result<(), String> {
    let config = ServerConfig::from_cli(Cli::parse())?;
    let service = CommandService::new(&config)?;
    let address = SocketAddr::new(config.listen, config.port);
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("Cannot listen on {address}: {error}"))?;

    let access_token = config.access_token.clone().map(Arc::<str>::from);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::unbounded_channel();
    let app = api::router(ApiState {
        service: service.clone(),
        auth_mode: config.auth_mode,
        access_token,
        shutdown: shutdown_tx.clone(),
    });

    println!("Tunnel Helper server is running");
    println!("Local UI: http://{}:{}", config.display_host(), config.port);
    if config.listen.is_unspecified() {
        if let Ok(interfaces) = if_addrs::get_if_addrs() {
            for interface in interfaces {
                if let std::net::IpAddr::V4(ip) = interface.ip() {
                    if !ip.is_loopback() && !ip.is_link_local() {
                        println!("Network UI: http://{ip}:{}", config.port);
                    }
                }
            }
        }
    }
    println!("Data dir: {}", service.data_dir().display());
    match (&config.auth_mode, &config.access_token) {
        (AuthMode::Disabled, _) => {
            println!("Access Token: disabled; anyone able to reach this service can control it.")
        }
        (AuthMode::Token, Some(token)) => println!("Access Token: {token}"),
        _ => {}
    }
    println!("Press Ctrl+C to stop the service and all managed tunnels.");

    #[cfg(target_os = "linux")]
    let (tray_shutdown, _tray_handle) = tray::start(config.display_host(), config.port).await;

    #[cfg(windows)]
    let _tray_handle = {
        let tray = windows_tray::start(config.display_host(), config.port, config.access_token.clone().unwrap_or_default(), shutdown_tx, service.clone());
        if tray.is_none() {
            if !_console_available && !windows_tray::allocate_fallback_console() {
                return Err("System tray and fallback console are unavailable; service was not started.".to_string());
            }
            eprintln!("System tray unavailable; the console remains open.");
        }
        tray
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            #[cfg(target_os = "linux")]
            tokio::select! {
                _ = shutdown_signal() => {},
                _ = tray_shutdown => {},
                _ = shutdown_rx.recv() => {},
            }
            #[cfg(not(target_os = "linux"))]
            tokio::select! {
                _ = shutdown_signal() => {},
                _ = shutdown_rx.recv() => {},
            }
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
