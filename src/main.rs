mod config;
mod handlers;
mod jmap;
mod log;
mod session;
mod templates;

use std::sync::Arc;

use config::Config;
use handlers::AppState;

fn main() {
    log_info!("Starting rust-jmap-webmail server");

    let config = match Config::load("config.toml") {
        Ok(c) => {
            log_info!("Configuration loaded from config.toml");
            c
        }
        Err(e) => {
            log_error!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let listen_addr = config.listen_address();
    log_info!("JMAP server URL: {}", config.jmap.well_known_url);
    log_info!("Binding to http://{}", listen_addr);

    let server = match tiny_http::Server::http(&listen_addr) {
        Ok(s) => {
            log_info!("HTTP server started successfully");
            s
        }
        Err(e) => {
            log_error!("Failed to start server: {}", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState::new(config));
    log_info!("Server ready, waiting for requests...");

    for request in server.incoming_requests() {
        let state = Arc::clone(&state);
        handlers::handle_request(&state, request);
    }
}
