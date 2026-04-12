use serde::{Deserialize, Serialize};

/// Configuration for the MCP server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub transport: McpTransport,
}

/// Transport protocol used to expose the MCP server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Standard input/output — default for local IDE integration.
    #[default]
    Stdio,
    /// Server-Sent Events over HTTP — for remote clients.
    Sse,
}
