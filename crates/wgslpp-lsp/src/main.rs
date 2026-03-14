mod completion;
mod diagnostics;
mod folding;
mod formatting;
mod hover;
mod navigation;
mod semantic_tokens;
mod server;
mod symbols;
mod workspace;

use lsp_server::Connection;
use lsp_types::InitializeParams;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    log::info!("wgslpp LSP server starting...");

    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(server::capabilities()).unwrap();

    let init_params = match connection.initialize(server_capabilities) {
        Ok(params) => params,
        Err(e) => {
            log::error!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    };

    let init_params: InitializeParams = serde_json::from_value(init_params).unwrap();

    if let Err(e) = server::run(&connection, init_params) {
        log::error!("Server error: {}", e);
        std::process::exit(1);
    }

    io_threads.join().unwrap();
}
