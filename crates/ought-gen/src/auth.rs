//! Local auth storage for generator providers that use OAuth.

use std::path::{Path, PathBuf};

use oharness_providers::{OpenAiCodexCredentials, OpenAiCodexOAuthClient};
use serde::{Deserialize, Serialize};

const AUTH_ENV: &str = "OUGHT_AUTH_FILE";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthFile {
    #[serde(default, rename = "openai-codex")]
    pub openai_codex: Option<OpenAiCodexCredentials>,
}

impl AuthFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read auth file {}: {}", path.display(), e))?;
        serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse auth file {}: {}", path.display(), e))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!("failed to create auth dir {}: {}", parent.display(), e)
            })?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
            .map_err(|e| anyhow::anyhow!("failed to write auth file {}: {}", path.display(), e))?;
        set_user_only_permissions(path)?;
        Ok(())
    }
}

pub fn auth_path(configured: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(path) = configured {
        return Ok(path.to_path_buf());
    }
    if let Ok(path) = std::env::var(AUTH_ENV)
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    home_dir()
        .map(|home| home.join(".ought").join("auth.json"))
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory for auth file"))
}

pub async fn load_openai_codex_credentials(
    configured_path: Option<&Path>,
) -> anyhow::Result<(PathBuf, OpenAiCodexCredentials)> {
    let path = auth_path(configured_path)?;
    let mut auth = AuthFile::load(&path)?;
    let mut credentials = auth.openai_codex.clone().ok_or_else(|| {
        anyhow::anyhow!("OpenAI Codex is not logged in; run `ought auth login openai-codex` first")
    })?;

    if credentials.is_expired() {
        let refreshed = OpenAiCodexOAuthClient::new()
            .refresh_access_token(&credentials.refresh)
            .await
            .map_err(|e| anyhow::anyhow!("failed to refresh OpenAI Codex token: {}", e))?;
        auth.openai_codex = Some(refreshed.clone());
        auth.save(&path)?;
        credentials = refreshed;
    }

    Ok((path, credentials))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(unix)]
fn set_user_only_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|e| {
        anyhow::anyhow!(
            "failed to set auth file permissions on {}: {}",
            path.display(),
            e
        )
    })
}

#[cfg(not(unix))]
fn set_user_only_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_path_uses_explicit_path() {
        let path = Path::new("/tmp/custom-auth.json");
        assert_eq!(auth_path(Some(path)).unwrap(), path);
    }
}
