//! App manifest: what the marketplace knows about one installable app.
//!
//! A manifest embeds a Docker Compose document. hearth runs it with
//! `docker compose`, passing these variables for interpolation:
//!
//! | Variable | Meaning |
//! |---|---|
//! | `${APP_DATA}` | per-app directory on the internal disk (configs, databases) |
//! | `${VAULT}` | bulk-storage root (external SSD) |
//! | `${TS_HOST}` | the machine's Tailscale `MagicDNS` name |
//! | `${TZ}` | time zone |
//!
//! ```
//! use hearth_core::AppManifest;
//!
//! let m = AppManifest::from_yaml(r#"
//! id: uptime-kuma
//! name: Uptime Kuma
//! description: Service monitoring
//! category: monitoring
//! port: 3002
//! upstream: { service: uptime-kuma, port: 3001 }
//! resources: { ram_mb: 150 }
//! compose: |
//!   services:
//!     uptime-kuma:
//!       image: louislam/uptime-kuma:2
//! "#).unwrap();
//! assert_eq!(m.resources.ram_mb, 150);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while parsing or validating a manifest.
#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("invalid manifest YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("manifest `{id}`: {reason}")]
    Invalid { id: String, reason: String },
}

/// One installable app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppManifest {
    /// Stable identifier: lowercase letters, digits and `-`. Also the compose project suffix.
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// Public HTTPS port on the tailnet (`https://<TS_HOST>:<port>`).
    pub port: u16,
    /// Where the gateway forwards traffic inside the `hearth` Docker network.
    pub upstream: Upstream,
    #[serde(default)]
    pub resources: Resources,
    #[serde(default)]
    pub auth: AuthPolicy,
    /// Upstream project homepage, shown in the marketplace.
    #[serde(default)]
    pub website: Option<String>,
    /// GitHub `owner/name`, used for popularity stats (stars, activity).
    #[serde(default)]
    pub repo: Option<String>,
    /// Docker Hub `namespace/name`, used for pull counts when available.
    #[serde(default)]
    pub docker_hub: Option<String>,
    /// Logo file name in `<catalog>/icons/` (defaults to `<id>.svg`).
    #[serde(default)]
    pub icon: Option<String>,
    /// Editor's pick: shown in the "Must-Have" section.
    #[serde(default)]
    pub featured: bool,
    /// One-line editorial blurb for featured placements.
    #[serde(default)]
    pub editors_note: Option<String>,
    /// Docker Compose YAML. Must define `upstream.service`.
    pub compose: String,
}

/// Gateway target for an app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    /// Compose service name (resolvable on the shared network).
    pub service: String,
    /// Container port the service listens on.
    pub port: u16,
}

/// Declared resource needs, used by the capacity planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// Typical steady-state RAM in MiB.
    #[serde(default)]
    pub ram_mb: u64,
    /// Expected bulk-storage growth in GiB (0 if the app stores little).
    #[serde(default)]
    pub vault_gb: u64,
    /// True if the app needs the external SSD (`${VAULT}`) to be mounted.
    #[serde(default)]
    pub needs_vault: bool,
}

/// How the gateway applies the single sign-on wall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthPolicy {
    /// Put the app behind Authelia. Defaults to `true`.
    #[serde(default = "default_true")]
    pub protect: bool,
    /// Path patterns (Caddy syntax) that skip Authelia, e.g. mobile-app APIs.
    #[serde(default)]
    pub exempt_paths: Vec<String>,
}

impl Default for AuthPolicy {
    fn default() -> Self {
        Self {
            protect: true,
            exempt_paths: Vec::new(),
        }
    }
}

const fn default_true() -> bool {
    true
}

/// Ports hearth itself or the base stack occupies; apps may not claim them.
pub const RESERVED_PORTS: &[u16] = &[53, 9091, 9000];

impl AppManifest {
    /// Parse and validate a manifest from YAML.
    pub fn from_yaml(src: &str) -> Result<Self, ManifestError> {
        let m: Self = serde_yaml_ng::from_str(src)?;
        m.validate()?;
        Ok(m)
    }

    /// Check invariants that serde cannot express.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let invalid = |reason: &str| ManifestError::Invalid {
            id: self.id.clone(),
            reason: reason.to_owned(),
        };

        let id_ok = !self.id.is_empty()
            && self.id.len() <= 48
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !self.id.starts_with('-');
        if !id_ok {
            return Err(invalid(
                "id must be 1-48 chars of [a-z0-9-], not starting with '-'",
            ));
        }
        if self.port < 1024 || RESERVED_PORTS.contains(&self.port) {
            return Err(invalid("port must be >= 1024 and not reserved"));
        }
        if self
            .auth
            .exempt_paths
            .iter()
            .any(|p| !p.starts_with('/') || p.contains(char::is_whitespace))
        {
            return Err(invalid(
                "exempt_paths must start with '/' and contain no spaces",
            ));
        }

        let compose: serde_yaml_ng::Value = serde_yaml_ng::from_str(&self.compose)
            .map_err(|e| invalid(&format!("compose is not valid YAML: {e}")))?;
        let has_service = compose
            .get("services")
            .and_then(|s| s.get(self.upstream.service.as_str()))
            .is_some();
        if !has_service {
            return Err(invalid("compose must define the upstream service"));
        }
        if self.compose.contains("${VAULT}") && !self.resources.needs_vault {
            return Err(invalid(
                "compose uses ${VAULT}; set resources.needs_vault: true",
            ));
        }
        if let Some(repo) = &self.repo
            && (repo.split('/').count() != 2 || repo.contains(char::is_whitespace))
        {
            return Err(invalid("repo must look like `owner/name`"));
        }
        Ok(())
    }

    /// Storage roles this app depends on (from `${VAULT}`/`${MEDIA}`/`${BACKUPS}` in compose).
    #[must_use]
    pub fn roles(&self) -> Vec<crate::storage::Role> {
        crate::storage::Role::ALL
            .into_iter()
            .filter(|r| self.compose.contains(&format!("${{{}}}", r.env_var())))
            .collect()
    }

    /// Logo file name inside the catalog's `icons/` directory.
    #[must_use]
    pub fn icon_file(&self) -> String {
        self.icon
            .clone()
            .unwrap_or_else(|| format!("{}.svg", self.id))
    }

    /// Docker Compose project name used for this app.
    #[must_use]
    pub fn project_name(&self) -> String {
        format!("hearth-{}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> String {
        r"
id: demo
name: Demo
description: d
category: tools
port: 4000
upstream: { service: web, port: 80 }
compose: |
  services:
    web:
      image: nginx
"
        .to_owned()
    }

    #[test]
    fn parses_minimal_manifest_with_defaults() {
        let m = AppManifest::from_yaml(&base()).expect("valid manifest");
        assert!(m.auth.protect, "auth defaults to protected");
        assert_eq!(m.auth.exempt_paths, Vec::<String>::new());
        assert_eq!(m.resources, Resources::default());
        assert_eq!(m.project_name(), "hearth-demo");
    }

    #[test]
    fn rejects_bad_id() {
        let src = base().replace("id: demo", "id: Demo_App");
        assert!(matches!(
            AppManifest::from_yaml(&src),
            Err(ManifestError::Invalid { .. })
        ));
    }

    #[test]
    fn rejects_reserved_or_privileged_port() {
        for p in ["53", "80", "9091"] {
            let src = base().replace("port: 4000", &format!("port: {p}"));
            assert!(
                AppManifest::from_yaml(&src).is_err(),
                "port {p} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_missing_upstream_service() {
        let src = base().replace("service: web", "service: api");
        assert!(AppManifest::from_yaml(&src).is_err());
    }

    #[test]
    fn requires_needs_vault_flag_when_vault_used() {
        let src = base().replace(
            "image: nginx",
            "image: nginx\n      volumes: [\"${VAULT}/x:/x\"]",
        );
        assert!(AppManifest::from_yaml(&src).is_err());
        let fixed = format!("{src}resources: {{ needs_vault: true }}\n");
        assert!(AppManifest::from_yaml(&fixed).is_ok());
    }

    #[test]
    fn roles_detected_from_compose() {
        let src = base().replace(
            "image: nginx",
            "image: nginx\n      volumes: [\"${VAULT}/a:/a\", \"${MEDIA}:/m\"]",
        ) + "resources: { needs_vault: true }\n";
        let m = AppManifest::from_yaml(&src).expect("valid");
        assert_eq!(
            m.roles(),
            [crate::storage::Role::Vault, crate::storage::Role::Media]
        );
        assert_eq!(m.icon_file(), "demo.svg");
    }

    #[test]
    fn rejects_bad_repo() {
        let src = format!("{}repo: not-a-repo\n", base());
        assert!(AppManifest::from_yaml(&src).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let src = format!("{}typo_field: 1\n", base());
        assert!(matches!(
            AppManifest::from_yaml(&src),
            Err(ManifestError::Yaml(_))
        ));
    }
}
