mod config;
mod handlers;
mod jmap;
mod session;
mod templates;

use std::sync::Arc;

use config::Config;
use handlers::AppState;

fn main() {
    let config = match Config::load("config.toml") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let listen_addr = config.listen_address();
    println!("Starting server on http://{}", listen_addr);

    let server = match tiny_http::Server::http(&listen_addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState::new(config));

    for request in server.incoming_requests() {
        let state = Arc::clone(&state);
        handlers::handle_request(&state, request);
    }
}
