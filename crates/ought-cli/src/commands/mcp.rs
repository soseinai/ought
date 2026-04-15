use super::{load_config, resolve_spec_roots};
use crate::{Cli, McpCommand, TransportArg};

pub fn run(cli: &Cli, command: &McpCommand) -> anyhow::Result<()> {
    match command {
        McpCommand::Serve { transport, port } => {
            let (config_path, config) = load_config(&cli.config)?;
            let project_root = config_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let spec_roots = resolve_spec_roots(&config, &config_path);
            let server = ought_mcp::server::McpServer::new(
                project_root,
                spec_roots,
                config.runner.clone(),
            );
            let server_transport = match transport {
                TransportArg::Stdio => ought_mcp::server::Transport::Stdio,
                TransportArg::Sse => ought_mcp::server::Transport::Sse {
                    port: port.unwrap_or(19877),
                },
            };
            tokio::runtime::Runtime::new()?.block_on(server.serve(server_transport))
        }
        McpCommand::Install => {
            ought_mcp::server::McpServer::install()?;
            eprintln!("Registered ought with MCP-compatible coding agents.");
            Ok(())
        }
    }
}
