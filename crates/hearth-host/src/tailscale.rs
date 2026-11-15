//! Tailscale via the host `tailscale` CLI: status, login/logout, hostname,
//! and `tailscale serve` publishing.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;

use crate::{CommandError, run};

#[derive(Debug, Clone, Default, Serialize)]
pub struct TailscaleStatus {
    pub installed: bool,
    pub version: Option<String>,
    /// `Running`, `NeedsLogin`, `Stopped`, `Starting`…
    pub backend_state: String,
    /// Present while a login is waiting for the user.
    pub auth_url: Option<String>,
    /// e.g. `m1.tail1234.ts.net` (no trailing dot)
    pub dns_name: Option<String>,
    pub hostname: Option<String>,
    pub ips: Vec<String>,
    pub tailnet: Option<String>,
    pub magic_dns: bool,
    /// HTTPS certificates are enabled for the tailnet (needed for `serve --https`).
    pub https_enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TailscaleError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("unexpected tailscale output: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid hostname: use 1-63 letters, digits or '-'")]
    BadHostname,
}

/// `tailscale status --json`, reduced to what hearth needs. Never fails if the CLI is missing.
pub async fn status() -> Result<TailscaleStatus, TailscaleError> {
    match run("tailscale", &["status", "--json"], &[]).await {
        Ok(raw) => Ok(parse_status(&raw)?),
        Err(CommandError::Spawn { .. }) => Ok(TailscaleStatus {
            backend_state: "NotInstalled".into(),
            ..TailscaleStatus::default()
        }),
        // Logged out / stopped daemons still print JSON on stdout but exit non-zero sometimes.
        Err(e @ CommandError::Failed { .. }) => Err(e.into()),
    }
}

fn parse_status(raw: &str) -> Result<TailscaleStatus, serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    let this = &v["Self"];
    let s = |x: &serde_json::Value| x.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    Ok(TailscaleStatus {
        installed: true,
        version: s(&v["Version"]),
        backend_state: v["BackendState"].as_str().unwrap_or("Unknown").to_owned(),
        auth_url: s(&v["AuthURL"]),
        dns_name: this["DNSName"]
            .as_str()
            .map(|s| s.trim_end_matches('.').to_owned())
            .filter(|s| !s.is_empty()),
        hostname: s(&this["HostName"]),
        ips: this["TailscaleIPs"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        tailnet: s(&v["CurrentTailnet"]["Name"]),
        magic_dns: v["CurrentTailnet"]["MagicDNSEnabled"]
            .as_bool()
            .unwrap_or(false),
        https_enabled: v["CertDomains"].as_array().is_some_and(|a| !a.is_empty()),
    })
}

/// Start an interactive login in the background. The URL to open shows up as
/// `auth_url` in [`status`] a moment later.
pub fn start_login() {
    tokio::spawn(async {
        let child = tokio::process::Command::new(crate::resolve("tailscale"))
            .arg("login")
            .kill_on_drop(true)
            .output();
        match tokio::time::timeout(Duration::from_secs(600), child).await {
            Ok(Ok(out)) if out.status.success() => tracing::info!("tailscale login completed"),
            Ok(Ok(out)) => {
                tracing::warn!(stderr = %String::from_utf8_lossy(&out.stderr), "tailscale login failed");
            }
            Ok(Err(e)) => tracing::warn!(error = %e, "could not run tailscale login"),
            Err(_) => tracing::warn!("tailscale login timed out"),
        }
    });
}

/// Log in to a custom control server (Headscale) with a pre-auth key. Non-interactive.
pub async fn login_with(login_server: &str, auth_key: &str) -> Result<(), TailscaleError> {
    let ok_url =
        login_server.starts_with("https://") && !login_server.contains(char::is_whitespace);
    let ok_key = !auth_key.is_empty() && !auth_key.contains(char::is_whitespace);
    if !ok_url || !ok_key {
        return Err(TailscaleError::BadHostname);
    }
    let server = format!("--login-server={login_server}");
    let key = format!("--auth-key={auth_key}");
    run("tailscale", &["login", &server, &key], &[]).await?;
    Ok(())
}

pub async fn logout() -> Result<(), TailscaleError> {
    run("tailscale", &["logout"], &[]).await?;
    Ok(())
}

/// Bring the connection up/down without logging out.
pub async fn set_up(up: bool) -> Result<(), TailscaleError> {
    run("tailscale", &[if up { "up" } else { "down" }], &[]).await?;
    Ok(())
}

pub async fn set_hostname(name: &str) -> Result<(), TailscaleError> {
    let ok = !name.is_empty()
        && name.len() <= 63
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !name.starts_with('-');
    if !ok {
        return Err(TailscaleError::BadHostname);
    }
    let arg = format!("--hostname={name}");
    run("tailscale", &["set", &arg], &[]).await?;
    Ok(())
}

/// Publish `https://<node>:<port>` → `http://127.0.0.1:<port>` (idempotent).
pub async fn serve(port: u16) -> Result<(), TailscaleError> {
    let https = format!("--https={port}");
    let target = format!("http://127.0.0.1:{port}");
    run("tailscale", &["serve", "--bg", &https, &target], &[]).await?;
    Ok(())
}

/// Stop publishing a port.
pub async fn unserve(port: u16) -> Result<(), TailscaleError> {
    let https = format!("--https={port}");
    run("tailscale", &["serve", &https, "off"], &[]).await?;
    Ok(())
}

/// Public internet access via Tailscale Funnel: `https://<node>` (443) → local port.
pub async fn funnel(local_port: u16) -> Result<(), TailscaleError> {
    let target = format!("http://127.0.0.1:{local_port}");
    run(
        "tailscale",
        &["funnel", "--bg", "--https=443", &target],
        &[],
    )
    .await?;
    Ok(())
}

pub async fn funnel_off() -> Result<(), TailscaleError> {
    run("tailscale", &["funnel", "--https=443", "off"], &[]).await?;
    Ok(())
}

/// Remove every `tailscale serve` entry.
pub async fn serve_reset() -> Result<(), TailscaleError> {
    run("tailscale", &["serve", "reset"], &[]).await?;
    Ok(())
}

/// Currently published HTTPS ports → proxy target.
pub async fn serve_status() -> Result<BTreeMap<u16, String>, TailscaleError> {
    let raw = run("tailscale", &["serve", "status", "--json"], &[]).await?;
    Ok(parse_serve(&raw)?)
}

fn parse_serve(raw: &str) -> Result<BTreeMap<u16, String>, serde_json::Error> {
    let v: serde_json::Value = if raw.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(raw)?
    };
    let mut out = BTreeMap::new();
    if let Some(web) = v["Web"].as_object() {
        for (hostport, cfg) in web {
            let Some(port) = hostport
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok())
            else {
                continue;
            };
            let target = cfg["Handlers"]["/"]["Proxy"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            out.insert(port, target);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_self_node_and_tailnet_features() {
        let raw = r#"{"Version":"1.90.1","BackendState":"Running","AuthURL":"","CertDomains":["m1.tail1234.ts.net"],
          "CurrentTailnet":{"Name":"me@example.com","MagicDNSEnabled":true},
          "Self":{"HostName":"m1","DNSName":"m1.tail1234.ts.net.","TailscaleIPs":["100.64.0.1","fd7a::1"]},"Peer":{}}"#;
        let s = parse_status(raw).expect("parses");
        assert_eq!(s.backend_state, "Running");
        assert_eq!(s.dns_name.as_deref(), Some("m1.tail1234.ts.net"));
        assert!(s.magic_dns && s.https_enabled);
        assert!(s.auth_url.is_none(), "empty AuthURL treated as none");
        assert_eq!(s.ips.len(), 2);
    }

    #[test]
    fn tolerates_logged_out_node() {
        let s = parse_status(r#"{"BackendState":"NeedsLogin","AuthURL":"https://login.tailscale.com/a/x","Self":{"DNSName":""}}"#)
            .expect("parses");
        assert!(s.dns_name.is_none());
        assert_eq!(
            s.auth_url.as_deref(),
            Some("https://login.tailscale.com/a/x")
        );
        assert!(!s.https_enabled);
    }

    #[test]
    fn parses_serve_status() {
        let raw = r#"{"TCP":{"8081":{"HTTPS":true}},"Web":{"m1.ts.net:8081":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:8081"}}},
          "m1.ts.net:9443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:9443"}}}}}"#;
        let m = parse_serve(raw).expect("parses");
        assert_eq!(
            m.get(&8081).map(String::as_str),
            Some("http://127.0.0.1:8081")
        );
        assert_eq!(m.len(), 2);
        assert!(parse_serve("").expect("empty ok").is_empty());
    }
}
