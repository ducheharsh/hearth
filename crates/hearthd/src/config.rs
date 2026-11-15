//! `hearth.toml`: bootstrap only (where to listen, where state lives).
//! Everything users change at runtime lives in [`crate::settings`].

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use hearth_core::storage::Role;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Keep on loopback: the gateway is the way in.
    pub listen: SocketAddr,
    /// Directory of built-in app manifests (the curated marketplace).
    pub catalog_dir: PathBuf,
    /// hearth's state: settings, apps, gateway, volumes, discover cache.
    pub state_dir: PathBuf,
    /// Override the Tailscale name used in URLs (auto-detected when unset).
    pub ts_host: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 9000)),
            catalog_dir: PathBuf::from("catalog"),
            state_dir: PathBuf::from("~/.hearth"),
            ts_host: None,
        }
    }
}

impl Config {
    /// Load from `path` if it exists, otherwise defaults. Expands `~`.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut cfg: Self = if path.exists() {
            let src = std::fs::read_to_string(path)?;
            toml::from_str(&src).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?
        } else {
            Self::default()
        };
        cfg.catalog_dir = expand_home(&cfg.catalog_dir);
        cfg.state_dir = expand_home(&cfg.state_dir);
        Ok(cfg)
    }

    #[must_use]
    pub fn apps_dir(&self) -> PathBuf {
        self.state_dir.join("apps")
    }
    #[must_use]
    pub fn gateway_dir(&self) -> PathBuf {
        self.state_dir.join("gateway")
    }
    #[must_use]
    pub fn custom_dir(&self) -> PathBuf {
        self.state_dir.join("custom")
    }
    #[must_use]
    pub fn secrets_dir(&self) -> PathBuf {
        self.state_dir.join("secrets")
    }
    /// Stable symlink apps use for a storage role.
    #[must_use]
    pub fn role_link(&self, role: Role) -> PathBuf {
        self.state_dir.join("volumes").join(role.as_str())
    }
    #[must_use]
    pub fn icons_dir(&self) -> PathBuf {
        self.catalog_dir.join("icons")
    }
}

fn expand_home(p: &Path) -> PathBuf {
    match (p.strip_prefix("~"), std::env::var_os("HOME")) {
        (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => p.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_loopback_defaults() {
        let cfg: Config = toml::from_str("").expect("parses");
        assert_eq!(cfg.listen.port(), 9000);
        assert!(cfg.listen.ip().is_loopback());
    }

    #[test]
    fn unknown_keys_rejected() {
        assert!(toml::from_str::<Config>("lisen = \"x\"").is_err());
    }

    #[test]
    fn expands_tilde() {
        let p = expand_home(Path::new("~/.hearth"));
        assert!(!p.starts_with("~"));
        assert!(p.ends_with(".hearth"));
    }
}
