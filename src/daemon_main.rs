use claude_ntfy::daemon::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::main().await
}