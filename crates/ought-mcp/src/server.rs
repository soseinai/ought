use std::path::PathBuf;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::resources::ResourceHandler;
use crate::tools::ToolHandler;

/// MCP server for ought.
///
/// Exposes ought tools and resources over stdio or SSE transport
/// so AI assistants and IDE extensions can interact programmatically.
pub struct McpServer {
    config_path: PathBuf,
}

/// Transport protocol for the MCP server.
#[derive(Debug, Clone, Copy)]
pub enum Transport {
    Stdio,
    Sse { port: u16 },
}

/// Tool descriptor for initialization response.
fn tool_descriptors() -> Value {
    serde_json::json!({
        "tools": [
            {
                "name": "ought_run",
                "description": "Run specs and return structured test results",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec": { "type": "string", "description": "Optional spec name to run (runs all if omitted)" },
                        "clause_id": { "type": "string", "description": "Optional clause ID to run a single clause" }
                    }
                }
            },
            {
                "name": "ought_generate",
                "description": "Regenerate stale or specified clause tests",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec": { "type": "string", "description": "Optional spec name" },
                        "clause_id": { "type": "string", "description": "Optional clause ID" },
                        "force": { "type": "boolean", "description": "Force regeneration even if not stale" }
                    }
                }
            },
            {
                "name": "ought_check",
                "description": "Validate spec syntax and return any parse errors",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec": { "type": "string", "description": "Optional spec name to check (checks all if omitted)" }
                    }
                }
            },
            {
                "name": "ought_inspect",
                "description": "Return generated test code for a clause",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "clause_id": { "type": "string", "description": "The clause ID to inspect" }
                    },
                    "required": ["clause_id"]
                }
            },
            {
                "name": "ought_status",
                "description": "Return spec coverage summary with clause counts by status",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "ought_survey",
                "description": "Analyze source for uncovered behaviors",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Source paths to survey"
                        }
                    }
                }
            },
            {
                "name": "ought_audit",
                "description": "Cross-spec conflict and gap analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "ought_blame",
                "description": "Explain why a clause is failing",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "clause_id": { "type": "string", "description": "The clause ID to blame" }
                    },
                    "required": ["clause_id"]
                }
            },
            {
                "name": "ought_bisect",
                "description": "Find the breaking commit for a clause",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "clause_id": { "type": "string", "description": "The clause ID to bisect" },
                        "range": { "type": "string", "description": "Optional git revision range" }
                    },
                    "required": ["clause_id"]
                }
            }
        ]
    })
}

/// Resource descriptor for initialization response.
fn resource_descriptors() -> Value {
    serde_json::json!({
        "resources": [
            {
                "uri": "ought://specs",
                "name": "Spec list",
                "description": "List of all spec files with clause counts",
                "mimeType": "application/json"
            },
            {
                "uri": "ought://specs/{name}",
                "name": "Spec detail",
                "description": "Parsed clauses for a specific spec file",
                "mimeType": "application/json"
            },
            {
                "uri": "ought://results/latest",
                "name": "Latest results",
                "description": "Results from the most recent test run",
                "mimeType": "application/json"
            },
            {
                "uri": "ought://coverage",
                "name": "Coverage",
                "description": "Clause coverage map",
                "mimeType": "application/json"
            },
            {
                "uri": "ought://manifest",
                "name": "Manifest",
                "description": "Current generation manifest with hashes and timestamps",
                "mimeType": "application/json"
            }
        ]
    })
}

impl McpServer {
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    /// Start serving on the given transport. Blocks until shutdown.
    pub async fn serve(self, transport: Transport) -> anyhow::Result<()> {
        match transport {
            Transport::Stdio => self.serve_stdio().await,
            Transport::Sse { port } => {
                anyhow::bail!("SSE transport on port {} is not yet implemented", port)
            }
        }
    }

    /// Serve over stdio: read JSON-RPC requests line by line from stdin,
    /// write JSON-RPC responses to stdout.
    async fn serve_stdio(self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        let tool_handler = ToolHandler::new(self.config_path.clone());
        let resource_handler = ResourceHandler::new(self.config_path.clone());

        while let Some(line) = lines.next_line().await? {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let response = Self::handle_request(&line, &tool_handler, &resource_handler);

            let response_str = serde_json::to_string(&response)
                .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"internal serialization error"}}"#.to_string());

            stdout
                .write_all(response_str.as_bytes())
                .await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    /// Parse a JSON-RPC request and route to the appropriate handler.
    pub fn handle_request(
        raw: &str,
        tool_handler: &ToolHandler,
        resource_handler: &ResourceHandler,
    ) -> Value {
        let req: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                return jsonrpc_error(Value::Null, -32700, "Parse error");
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = match req.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                return jsonrpc_error(id, -32600, "Invalid Request: missing method");
            }
        };
        let params = req.get("params").cloned().unwrap_or(serde_json::json!({}));

        match method {
            "initialize" => Self::handle_initialize(id),
            "tools/list" => Self::handle_tools_list(id),
            "resources/list" => Self::handle_resources_list(id),
            "tools/call" => Self::handle_tool_call(id, &params, tool_handler),
            "resources/read" => {
                Self::handle_resource_read(id, &params, resource_handler)
            }
            "notifications/initialized" => {
                // Client notification, no response needed (but we return one for simplicity)
                serde_json::json!(null)
            }
            _ => jsonrpc_error(id, -32601, &format!("Method not found: {}", method)),
        }
    }

    fn handle_initialize(id: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "ought",
                    "version": "0.1.0"
                }
            }
        })
    }

    fn handle_tools_list(id: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": tool_descriptors()
        })
    }

    fn handle_resources_list(id: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": resource_descriptors()
        })
    }

    fn handle_tool_call(id: Value, params: &Value, handler: &ToolHandler) -> Value {
        let tool_name = match params.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => {
                return jsonrpc_error(id, -32602, "Invalid params: missing tool name");
            }
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let result = match tool_name {
            "ought_run" => handler.ought_run(args),
            "ought_generate" => handler.ought_generate(args),
            "ought_check" => handler.ought_check(args),
            "ought_inspect" => handler.ought_inspect(args),
            "ought_status" => handler.ought_status(args),
            "ought_survey" => handler.ought_survey(args),
            "ought_audit" => handler.ought_audit(args),
            "ought_blame" => handler.ought_blame(args),
            "ought_bisect" => handler.ought_bisect(args),
            _ => {
                return jsonrpc_error(
                    id,
                    -32602,
                    &format!("Unknown tool: {}", tool_name),
                );
            }
        };

        match result {
            Ok(value) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}]
                }
            }),
            Err(e) => jsonrpc_error(id, -32000, &format!("{:#}", e)),
        }
    }

    fn handle_resource_read(
        id: Value,
        params: &Value,
        handler: &ResourceHandler,
    ) -> Value {
        let uri = match params.get("uri").and_then(|u| u.as_str()) {
            Some(u) => u,
            None => {
                return jsonrpc_error(id, -32602, "Invalid params: missing resource URI");
            }
        };

        let result = if uri == "ought://specs" {
            handler.specs_list()
        } else if let Some(name) = uri.strip_prefix("ought://specs/") {
            handler.specs_get(name)
        } else if uri == "ought://results/latest" {
            handler.results_latest()
        } else if uri == "ought://coverage" {
            handler.coverage()
        } else if uri == "ought://manifest" {
            handler.manifest()
        } else {
            return jsonrpc_error(
                id,
                -32602,
                &format!("Unknown resource URI: {}", uri),
            );
        };

        match result {
            Ok(value) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "contents": [{"uri": uri, "mimeType": "application/json", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}]
                }
            }),
            Err(e) => jsonrpc_error(id, -32000, &format!("{:#}", e)),
        }
    }

    /// Register ought with MCP-compatible coding agents
    /// (Claude Code, Codex, OpenCode).
    pub fn install() -> anyhow::Result<()> {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;

        // Write Claude Code MCP config
        let claude_dir = PathBuf::from(&home).join(".claude");
        std::fs::create_dir_all(&claude_dir)?;

        let mcp_config_path = claude_dir.join("mcp.json");

        let mcp_config = serde_json::json!({
            "mcpServers": {
                "ought": {
                    "command": "ought",
                    "args": ["mcp", "serve"],
                    "transport": "stdio"
                }
            }
        });

        // If the file already exists, merge rather than overwrite
        let existing: Value = if mcp_config_path.exists() {
            let content = std::fs::read_to_string(&mcp_config_path)?;
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let mut merged = existing;
        if let Some(obj) = merged.as_object_mut() {
            let servers = obj
                .entry("mcpServers")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(servers_obj) = servers.as_object_mut() {
                servers_obj.insert(
                    "ought".to_string(),
                    serde_json::json!({
                        "command": "ought",
                        "args": ["mcp", "serve"],
                        "transport": "stdio"
                    }),
                );
            }
        } else {
            merged = mcp_config;
        }

        let content = serde_json::to_string_pretty(&merged)?;
        std::fs::write(&mcp_config_path, content)?;

        Ok(())
    }
}

/// Build a JSON-RPC error response.
fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}
