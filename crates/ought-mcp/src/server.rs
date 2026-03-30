use std::path::PathBuf;

/// MCP server for ought.
///
/// Exposes ought tools and resources over stdio or SSE transport
/// so AI assistants and IDE extensions can interact programmatically.
pub struct McpServer {
    _config_path: PathBuf,
}

/// Transport protocol for the MCP server.
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    Stdio,
    Sse { port: u16 },
}

impl McpServer {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            _config_path: config_path,
        }
    }

    /// Start serving on the given transport. Blocks until shutdown.
    pub async fn serve(self, _transport: Transport) -> anyhow::Result<()> {
        todo!()
    }

    /// Register ought with MCP-compatible coding agents
    /// (Claude Code, Codex, OpenCode).
    pub fn install() -> anyhow::Result<()> {
        todo!()
    }
}
