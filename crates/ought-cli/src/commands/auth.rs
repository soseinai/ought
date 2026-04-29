use std::io::Write;
use std::process::Command;

use oharness_providers::OpenAiCodexOAuthClient;
use ought_gen::auth::{AuthFile, auth_path};

use crate::{AuthCommand, AuthProvider};

pub fn run(command: &AuthCommand) -> anyhow::Result<()> {
    match command {
        AuthCommand::Login(args) => match args.provider {
            AuthProvider::OpenAiCodex => login_openai_codex(),
        },
        AuthCommand::Status => status(),
        AuthCommand::Logout(args) => match args.provider {
            AuthProvider::OpenAiCodex => logout_openai_codex(),
        },
    }
}

fn login_openai_codex() -> anyhow::Result<()> {
    let path = auth_path(None)?;
    let mut auth_file = AuthFile::load(&path)?;
    let client = OpenAiCodexOAuthClient::new();
    let authorization = client.create_authorization();
    let server = client.start_callback_server(authorization.state.clone());

    eprintln!("OpenAI Codex sign-in");
    eprintln!("Open this URL in your browser:\n\n{}\n", authorization.url);
    if try_open_browser(&authorization.url) {
        eprintln!("Opened browser. Waiting for sign-in to finish...");
    } else {
        eprintln!("Could not open a browser automatically.");
    }

    let code = match server {
        Ok(server) => match server.wait_for_code() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Browser callback failed: {e}");
                prompt_for_code(&authorization.state)?
            }
        },
        Err(e) => {
            eprintln!("Could not start localhost callback server: {e}");
            prompt_for_code(&authorization.state)?
        }
    };

    let credentials = tokio::runtime::Runtime::new()?
        .block_on(client.exchange_authorization_code(&code, &authorization.verifier))
        .map_err(|e| anyhow::anyhow!("OpenAI Codex token exchange failed: {}", e))?;

    auth_file.openai_codex = Some(credentials);
    auth_file.save(&path)?;
    eprintln!("Saved OpenAI Codex credentials to {}", path.display());
    Ok(())
}

fn status() -> anyhow::Result<()> {
    let path = auth_path(None)?;
    let auth_file = AuthFile::load(&path)?;
    eprintln!("Auth file: {}", path.display());
    if let Some(credentials) = auth_file.openai_codex {
        let state = if credentials.is_expired() {
            "expired"
        } else {
            "valid"
        };
        eprintln!(
            "openai-codex: logged in ({state}, expires {})",
            format_expires(credentials.expires)
        );
    } else {
        eprintln!("openai-codex: not logged in");
    }
    Ok(())
}

fn logout_openai_codex() -> anyhow::Result<()> {
    let path = auth_path(None)?;
    let mut auth_file = AuthFile::load(&path)?;
    auth_file.openai_codex = None;
    auth_file.save(&path)?;
    eprintln!("Removed OpenAI Codex credentials from {}", path.display());
    Ok(())
}

fn prompt_for_code(expected_state: &str) -> anyhow::Result<String> {
    eprint!("Paste the authorization code or full redirect URL: ");
    std::io::stderr().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    OpenAiCodexOAuthClient::parse_authorization_input(&input, expected_state)
        .map_err(|e| anyhow::anyhow!("invalid authorization response: {}", e))
}

fn try_open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn format_expires(ms: u64) -> String {
    let Ok(ms) = i64::try_from(ms) else {
        return "unknown".to_string();
    };
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}
