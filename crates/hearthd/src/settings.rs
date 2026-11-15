//! Runtime settings, edited from the Settings page. Persisted in `state_dir/settings.json`.
//! Secrets (API tokens) are *not* stored here; see `secrets/`.

use std::collections::BTreeMap;

use hearth_core::CapacityPolicy;
use serde::{Deserialize, Serialize};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // independent user toggles
pub struct Settings {
    /// Shown in the header and browser title.
    pub display_name: String,
    /// IANA time zone passed to apps as `TZ`.
    pub tz: String,
    pub capacity: CapacitySettings,
    /// Only serve API requests that came through Authelia (`Remote-User` header).
    pub require_remote_user: bool,
    /// Run the Caddy + Authelia gateway and publish routes with `tailscale serve`.
    pub gateway_enabled: bool,
    /// Custom domain for subdomain routing (set by the Cloudflare setup).
    pub domain: Option<String>,
    pub cloudflare: CloudflareSettings,
    /// Sync the awesome-selfhosted directory for Discover and leaderboards.
    pub discover_enabled: bool,
    /// Stop apps when their drive disappears; start them again when it returns.
    pub auto_pause_on_disconnect: bool,
    /// Per-app overrides (port, subdomain, public).
    pub apps: BTreeMap<String, AppOverride>,
    pub tunnels: Tunnels,
    /// Also serve the gateway on this machine's NetBird address.
    pub netbird_enabled: bool,
    /// Who issues the tailnet HTTPS certificates.
    pub tailscale_certs: TsCerts,
    /// Keeping the Mac awake and connected.
    pub always_on: AlwaysOn,
}

/// Running as a server around the clock.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AlwaysOn {
    /// Hold a "prevent system sleep" assertion (honoured on power only).
    pub keep_awake: bool,
    /// Check the internet every 30 s and report outages.
    pub watch_connectivity: bool,
    /// Turn Wi-Fi back on, and reconnect Tailscale after an outage.
    pub auto_repair: bool,
}

impl Default for AlwaysOn {
    fn default() -> Self {
        Self {
            keep_awake: true,
            watch_connectivity: true,
            auto_repair: true,
        }
    }
}

/// HTTPS on the tailnet: Tailscale's own certificates (published with `tailscale serve`), or
/// hearth's certificate authority for tailnets where HTTPS certificates can't be turned on.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TsCerts {
    #[default]
    Tailscale,
    /// Caddy serves HTTPS on the Tailscale IP; each device trusts hearth's CA once.
    Hearth,
}

/// Single-app public tunnels (rely on the app's own login).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Tunnels {
    /// App published with Tailscale Funnel at `https://<node>`.
    pub funnel_app: Option<String>,
    /// App published with ngrok.
    pub ngrok_app: Option<String>,
    /// Your ngrok static domain, e.g. `name.ngrok-free.app`.
    pub ngrok_domain: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CapacitySettings {
    pub mem_reserve_mb: u64,
    pub mem_warn_headroom_mb: u64,
    pub disk_reserve_gb: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CloudflareSettings {
    pub account_id: Option<String>,
    pub zone_id: Option<String>,
    pub tunnel_id: Option<String>,
    pub dns_record_id: Option<String>,
    /// Connector container should be running.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppOverride {
    /// Tailnet port instead of the manifest's.
    pub port: Option<u16>,
    /// Subdomain label instead of the app id.
    pub subdomain: Option<String>,
    /// Reachable on the custom domain from the internet (still behind the login).
    pub public: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display_name: "hearth".into(),
            tz: "UTC".into(),
            capacity: CapacitySettings::default(),
            require_remote_user: false,
            gateway_enabled: false,
            domain: None,
            cloudflare: CloudflareSettings::default(),
            discover_enabled: true,
            auto_pause_on_disconnect: true,
            apps: BTreeMap::new(),
            tunnels: Tunnels::default(),
            netbird_enabled: false,
            tailscale_certs: TsCerts::default(),
            always_on: AlwaysOn::default(),
        }
    }
}

impl Default for CapacitySettings {
    fn default() -> Self {
        let p = CapacityPolicy::default();
        Self {
            mem_reserve_mb: p.mem_reserve / MIB,
            mem_warn_headroom_mb: p.mem_warn_headroom / MIB,
            disk_reserve_gb: p.disk_reserve / GIB,
        }
    }
}

impl CapacitySettings {
    #[must_use]
    pub fn policy(self) -> CapacityPolicy {
        CapacityPolicy {
            mem_reserve: self.mem_reserve_mb * MIB,
            mem_warn_headroom: self.mem_warn_headroom_mb * MIB,
            disk_reserve: self.disk_reserve_gb * GIB,
        }
    }
}

/// A DNS label: 1-63 chars of `[a-z0-9-]`, not starting/ending with `-`.
#[must_use]
pub fn valid_label(s: &str) -> bool {
    (1..=63).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// A domain like `example.com` or `home.example.com`.
#[must_use]
pub fn valid_domain(s: &str) -> bool {
    let labels: Vec<&str> = s.split('.').collect();
    labels.len() >= 2 && labels.iter().all(|l| valid_label(l))
}

impl Settings {
    /// Reject impossible values before saving.
    pub fn validate(&self) -> Result<(), String> {
        if self.display_name.trim().is_empty() || self.display_name.len() > 40 {
            return Err("Display name must be 1-40 characters.".into());
        }
        if self.tz.is_empty() || self.tz.contains(char::is_whitespace) {
            return Err("Time zone looks invalid (e.g. Asia/Kolkata).".into());
        }
        if self.capacity.mem_reserve_mb < 256 || self.capacity.disk_reserve_gb < 1 {
            return Err("Keep at least 256 MB RAM and 1 GB disk in reserve.".into());
        }
        if let Some(d) = &self.domain
            && !valid_domain(d)
        {
            return Err(format!("`{d}` isn't a valid domain."));
        }
        for (id, o) in &self.apps {
            if let Some(s) = &o.subdomain
                && !valid_label(s)
            {
                return Err(format!(
                    "{id}: subdomain must be lowercase letters, digits and '-'."
                ));
            }
            if let Some(p) = o.port
                && (p < 1024 || hearth_core::manifest::RESERVED_PORTS.contains(&p))
            {
                return Err(format!("{id}: port {p} is reserved; use 1024-65535."));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_old_files_still_load() {
        Settings::default().validate().expect("valid");
        let s: Settings =
            serde_json::from_str(r#"{"tz":"Asia/Kolkata"}"#).expect("partial file loads");
        assert_eq!(s.tz, "Asia/Kolkata");
        assert!(s.discover_enabled);
    }

    #[test]
    fn validation() {
        let mut s = Settings {
            domain: Some("bad domain".into()),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        s.domain = Some("home.example.com".into());
        s.validate().expect("ok");
        s.apps.insert(
            "x".into(),
            AppOverride {
                port: Some(53),
                ..AppOverride::default()
            },
        );
        assert!(s.validate().is_err());
        s.apps.insert(
            "x".into(),
            AppOverride {
                subdomain: Some("Bad_Sub".into()),
                ..AppOverride::default()
            },
        );
        assert!(s.validate().is_err());
    }

    #[test]
    fn labels() {
        assert!(valid_label("status-1"));
        assert!(!valid_label("-x") && !valid_label("UP") && !valid_label(""));
        assert!(valid_domain("a.io") && !valid_domain("localhost"));
    }
}
