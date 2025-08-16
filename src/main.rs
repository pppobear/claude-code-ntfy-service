use anyhow::Result;

mod cli;
mod config;
mod daemon;
mod errors;
mod hooks;
mod ntfy;
mod shared;

use cli::CliApp;

#[tokio::main]
async fn main() -> Result<()> {
    CliApp::run().await
}
