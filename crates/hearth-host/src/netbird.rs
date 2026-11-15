//! NetBird (open-source WireGuard mesh, like Tailscale) via the host `netbird` CLI.

use serde::Serialize;

use crate::{CommandError, run};

#[derive(Debug, Clone, Default, Serialize)]
pub struct NetbirdStatus {
    pub installed: bool,
    pub connected: bool,
    pub version: Option<String>,
    /// e.g. `100.92.14.3` (without the `/16`)
    pub ip: Option<String>,
    /// e.g. `m1.netbird.cloud`
    pub fqdn: Option<String>,
    pub management_url: Option<String>,
    pub peers_connected: u64,
    pub peers_total: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum NetbirdError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("unexpected netbird output: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid setup key or management URL")]
    BadInput,
}

pub async fn status() -> Result<NetbirdStatus, NetbirdError> {
    match run("netbird", &["status", "--json"], &[]).await {
        Ok(raw) => Ok(parse_status(&raw)?),
        Err(CommandError::Spawn { .. }) => Ok(NetbirdStatus::default()),
        // Daemon stopped / not logged in: installed but disconnected.
        Err(CommandError::Failed { .. } | CommandError::Timeout { .. }) => Ok(NetbirdStatus {
            installed: true,
            ..NetbirdStatus::default()
        }),
    }
}

fn parse_status(raw: &str) -> Result<NetbirdStatus, serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    let s = |x: &serde_json::Value| x.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    Ok(NetbirdStatus {
        installed: true,
        connected: v["management"]["connected"].as_bool().unwrap_or(false),
        version: s(&v["daemonVersion"]).or_else(|| s(&v["cliVersion"])),
        ip: s(&v["netbirdIp"]).map(|ip| ip.split('/').next().unwrap_or_default().to_owned()),
        fqdn: s(&v["fqdn"]).map(|f| f.trim_end_matches('.').to_owned()),
        management_url: s(&v["management"]["url"]),
        peers_connected: v["peers"]["connected"].as_u64().unwrap_or(0),
        peers_total: v["peers"]["total"].as_u64().unwrap_or(0),
    })
}

/// Connect with a setup key (optionally to a self-hosted management server).
pub async fn up(setup_key: &str, management_url: Option<&str>) -> Result<(), NetbirdError> {
    let key = setup_key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return Err(NetbirdError::BadInput);
    }
    let mut args = vec!["up", "--setup-key", key];
    if let Some(url) = management_url.map(str::trim).filter(|u| !u.is_empty()) {
        if !url.starts_with("https://") || url.contains(char::is_whitespace) {
            return Err(NetbirdError::BadInput);
        }
        args.extend(["--management-url", url]);
    }
    run("netbird", &args, &[]).await?;
    Ok(())
}

pub async fn down() -> Result<(), NetbirdError> {
    run("netbird", &["down"], &[]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status() {
        let raw = r#"{"peers":{"total":3,"connected":2,"details":[]},"cliVersion":"0.36.5","daemonVersion":"0.36.5",
          "management":{"url":"https://api.netbird.io:443","connected":true},"signal":{"connected":true},
          "netbirdIp":"100.92.14.3/16","fqdn":"m1.netbird.cloud"}"#;
        let s = parse_status(raw).expect("parses");
        assert!(s.connected);
        assert_eq!(s.ip.as_deref(), Some("100.92.14.3"));
        assert_eq!(s.fqdn.as_deref(), Some("m1.netbird.cloud"));
        assert_eq!((s.peers_connected, s.peers_total), (2, 3));
    }
}
