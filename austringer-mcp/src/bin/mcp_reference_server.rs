//! Standalone wrapper around the in-repo reference MCP server: serves
//! `ReferenceFsServer` on real stdin/stdout for stdio-transport interop
//! tests and manual protocol debugging. Root comes from
//! `AUSTRINGER_T29_FS_ROOT`.

use austringer_mcp::reference_server::ReferenceFsServer;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = std::env::var("AUSTRINGER_T29_FS_ROOT").unwrap_or_else(|_| ".".to_owned());
    eprintln!("mcp-reference-server: serving filesystem root {root}");
    let server = ReferenceFsServer::new(std::path::PathBuf::from(root));
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    match rmcp::service::serve_server(server, transport).await {
        Ok(service) => {
            let _ = service.waiting().await;
        }
        Err(error) => {
            eprintln!("mcp-reference-server: initialize failed: {error}");
        }
    }
}
