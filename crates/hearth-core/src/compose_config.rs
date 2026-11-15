//! Editable Docker settings for an app, derived from its compose file.
//!
//! The marketplace shows these as a form at install time. Untouched, they reproduce
//! the manifest's compose exactly (the recommended defaults). Edits are applied back
//! onto the YAML, so anything the form doesn't model is preserved.

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("compose is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("compose has no `services` map")]
    NoServices,
    #[error("unknown service `{0}`")]
    UnknownService(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    /// Looks like a secret; the UI masks it.
    #[serde(default)]
    pub secret: bool,
}

/// What the form edits for one service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    /// Image without tag, e.g. `louislam/uptime-kuma`.
    pub image: String,
    /// Tag, e.g. `2` or `latest`.
    pub tag: String,
    pub environment: Vec<EnvVar>,
    /// Short syntax, e.g. `53:53/udp`.
    pub ports: Vec<String>,
    /// Short syntax, e.g. `${APP_DATA}:/data`.
    pub volumes: Vec<String>,
    pub restart: String,
    /// e.g. `512m`, `1g`.
    pub mem_limit: Option<String>,
    /// e.g. `0.5`.
    pub cpus: Option<String>,
    /// Space-separated override of the image's command.
    pub command: Option<String>,
    pub privileged: bool,
    pub read_only: bool,
}

/// Saved form state: per-service edits, or a fully hand-written compose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AppConfig {
    Form { services: Vec<ServiceConfig> },
    Raw { compose: String },
}

pub const RESTART_POLICIES: &[&str] = &["no", "always", "unless-stopped", "on-failure"];
const SECRET_HINTS: &[&str] = &["PASS", "SECRET", "TOKEN", "KEY", "PRIVATE"];

fn is_secret(key: &str) -> bool {
    let k = key.to_ascii_uppercase();
    SECRET_HINTS.iter().any(|h| k.contains(h))
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_sequence)
        .map(|s| s.iter().filter_map(scalar).collect())
        .unwrap_or_default()
}

/// Split `repo/name:tag` (registry ports like `host:5000/img` handled).
#[must_use]
pub fn split_image(image: &str) -> (String, String) {
    let slash = image.rfind('/').map_or(0, |i| i + 1);
    match image[slash..].rfind(':') {
        Some(i) => (
            image[..slash + i].to_owned(),
            image[slash + i + 1..].to_owned(),
        ),
        None => (image.to_owned(), "latest".to_owned()),
    }
}

fn services(doc: &Value) -> Result<&Mapping, ConfigError> {
    doc.get("services")
        .and_then(Value::as_mapping)
        .ok_or(ConfigError::NoServices)
}

/// Read the editable settings of every service.
pub fn extract(compose: &str) -> Result<Vec<ServiceConfig>, ConfigError> {
    let doc: Value = serde_yaml_ng::from_str(compose)?;
    let mut out = Vec::new();
    for (name, svc) in services(&doc)? {
        let name = scalar(name).unwrap_or_default();
        let (image, tag) = split_image(svc.get("image").and_then(Value::as_str).unwrap_or(""));
        let environment = match svc.get("environment") {
            Some(Value::Mapping(m)) => m
                .iter()
                .map(|(k, v)| {
                    let key = scalar(k).unwrap_or_default();
                    EnvVar {
                        secret: is_secret(&key),
                        value: scalar(v).unwrap_or_default(),
                        key,
                    }
                })
                .collect(),
            Some(Value::Sequence(s)) => s
                .iter()
                .filter_map(Value::as_str)
                .map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                    EnvVar {
                        key: k.into(),
                        value: v.into(),
                        secret: is_secret(k),
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        let command = match svc.get("command") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Sequence(s)) => {
                Some(s.iter().filter_map(scalar).collect::<Vec<_>>().join(" "))
            }
            _ => None,
        };
        out.push(ServiceConfig {
            name,
            image,
            tag,
            environment,
            ports: str_list(svc.get("ports")),
            volumes: str_list(svc.get("volumes")),
            restart: svc
                .get("restart")
                .and_then(scalar)
                .unwrap_or_else(|| "unless-stopped".into()),
            mem_limit: svc.get("mem_limit").and_then(scalar),
            cpus: svc.get("cpus").and_then(scalar),
            command,
            privileged: svc
                .get("privileged")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            read_only: svc
                .get("read_only")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    Ok(out)
}

fn validate(c: &ServiceConfig) -> Result<(), ConfigError> {
    let bad = |m: &str| Err(ConfigError::Invalid(format!("{}: {m}", c.name)));
    if c.image.trim().is_empty() || c.image.contains(char::is_whitespace) {
        return bad("image is required and can't contain spaces");
    }
    if c.tag.trim().is_empty() || c.tag.contains(char::is_whitespace) {
        return bad("tag is required and can't contain spaces");
    }
    if !RESTART_POLICIES.contains(&c.restart.as_str()) && !c.restart.starts_with("on-failure:") {
        return bad("unknown restart policy");
    }
    if c.environment
        .iter()
        .any(|e| e.key.is_empty() || e.key.contains(['=', ' ']))
    {
        return bad("environment names can't be empty or contain '=' or spaces");
    }
    if c.ports
        .iter()
        .chain(&c.volumes)
        .any(|p| p.trim().is_empty())
    {
        return bad("empty port or volume entry");
    }
    if let Some(m) = &c.mem_limit
        && !m.is_empty()
        && !m
            .trim_end_matches(['b', 'k', 'm', 'g', 'B', 'K', 'M', 'G'])
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return bad("memory limit looks like 512m or 1g");
    }
    if let Some(cpu) = &c.cpus
        && !cpu.is_empty()
        && cpu.parse::<f64>().map_or(true, |v| v <= 0.0)
    {
        return bad("cpus must be a positive number");
    }
    Ok(())
}

fn set_or_remove(map: &mut Mapping, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            map.insert(Value::String(key.into()), v);
        }
        None => {
            map.remove(key);
        }
    }
}

fn seq(items: &[String]) -> Option<Value> {
    (!items.is_empty()).then(|| Value::Sequence(items.iter().cloned().map(Value::String).collect()))
}

/// Apply form edits onto the compose YAML (unmodelled keys are preserved).
pub fn apply(compose: &str, edits: &[ServiceConfig]) -> Result<String, ConfigError> {
    let mut doc: Value = serde_yaml_ng::from_str(compose)?;
    let svcs = doc
        .get_mut("services")
        .and_then(Value::as_mapping_mut)
        .ok_or(ConfigError::NoServices)?;
    for e in edits {
        validate(e)?;
        let svc = svcs
            .get_mut(e.name.as_str())
            .and_then(Value::as_mapping_mut)
            .ok_or_else(|| ConfigError::UnknownService(e.name.clone()))?;
        svc.insert(
            "image".into(),
            Value::String(format!("{}:{}", e.image.trim(), e.tag.trim())),
        );
        let env: Mapping = e
            .environment
            .iter()
            .map(|v| (Value::String(v.key.clone()), Value::String(v.value.clone())))
            .collect();
        set_or_remove(
            svc,
            "environment",
            (!env.is_empty()).then_some(Value::Mapping(env)),
        );
        set_or_remove(svc, "ports", seq(&e.ports));
        set_or_remove(svc, "volumes", seq(&e.volumes));
        svc.insert("restart".into(), Value::String(e.restart.clone()));
        let opt = |s: &Option<String>| {
            s.as_ref()
                .filter(|v| !v.trim().is_empty())
                .map(|v| Value::String(v.trim().into()))
        };
        set_or_remove(svc, "mem_limit", opt(&e.mem_limit));
        set_or_remove(svc, "cpus", opt(&e.cpus));
        set_or_remove(svc, "command", opt(&e.command));
        set_or_remove(svc, "privileged", e.privileged.then_some(Value::Bool(true)));
        set_or_remove(svc, "read_only", e.read_only.then_some(Value::Bool(true)));
    }
    Ok(serde_yaml_ng::to_string(&doc)?)
}

/// The compose to actually deploy for a saved config.
pub fn resolve(manifest_compose: &str, config: Option<&AppConfig>) -> Result<String, ConfigError> {
    match config {
        None => Ok(manifest_compose.to_owned()),
        Some(AppConfig::Form { services }) => apply(manifest_compose, services),
        Some(AppConfig::Raw { compose }) => {
            let doc: Value = serde_yaml_ng::from_str(compose)?;
            services(&doc)?;
            Ok(compose.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSE: &str = "services:
  web:
    image: ghcr.io/acme/web:1.2
    restart: unless-stopped
    environment:
      TZ: ${TZ}
      DB_PASSWORD: ${APP_SECRET}
    volumes:
      - ${APP_DATA}:/data
    healthcheck:
      test: [\"CMD\", \"true\"]
  cache:
    image: redis
    command: [\"redis-server\", \"--save\", \"60\", \"1\"]
";

    #[test]
    fn extracts_defaults() {
        let s = extract(COMPOSE).expect("ok");
        assert_eq!(s.len(), 2);
        let web = &s[0];
        assert_eq!(
            (web.image.as_str(), web.tag.as_str()),
            ("ghcr.io/acme/web", "1.2")
        );
        assert!(
            web.environment
                .iter()
                .any(|e| e.key == "DB_PASSWORD" && e.secret)
        );
        assert!(web.environment.iter().any(|e| e.key == "TZ" && !e.secret));
        let cache = &s[1];
        assert_eq!(cache.tag, "latest");
        assert_eq!(cache.restart, "unless-stopped", "default restart policy");
        assert_eq!(cache.command.as_deref(), Some("redis-server --save 60 1"));
    }

    #[test]
    fn untouched_roundtrip_keeps_everything() {
        let s = extract(COMPOSE).expect("ok");
        let out = apply(COMPOSE, &s).expect("apply");
        assert!(
            out.contains("healthcheck"),
            "unmodelled keys preserved:\n{out}"
        );
        assert_eq!(extract(&out).expect("ok")[0], s[0]);
    }

    #[test]
    fn edits_are_applied() {
        let mut s = extract(COMPOSE).expect("ok");
        s[0].tag = "1.3".into();
        s[0].environment.push(EnvVar {
            key: "LOG_LEVEL".into(),
            value: "debug".into(),
            secret: false,
        });
        s[0].ports = vec!["8080:80".into()];
        s[0].mem_limit = Some("512m".into());
        s[1].restart = "always".into();
        let out = apply(COMPOSE, &s).expect("apply");
        let again = extract(&out).expect("ok");
        assert_eq!(again[0].tag, "1.3");
        assert_eq!(again[0].ports, ["8080:80"]);
        assert_eq!(again[0].mem_limit.as_deref(), Some("512m"));
        assert!(again[0].environment.iter().any(|e| e.key == "LOG_LEVEL"));
        assert_eq!(again[1].restart, "always");
    }

    #[test]
    fn rejects_bad_values() {
        let base = extract(COMPOSE).expect("ok");
        for mutate in [
            (|c: &mut ServiceConfig| c.tag = "has space".into()) as fn(&mut ServiceConfig),
            |c| c.restart = "sometimes".into(),
            |c| c.mem_limit = Some("lots".into()),
            |c| c.cpus = Some("-1".into()),
            |c| {
                c.environment.push(EnvVar {
                    key: "A=B".into(),
                    value: String::new(),
                    secret: false,
                });
            },
        ] {
            let mut s = base.clone();
            mutate(&mut s[0]);
            assert!(apply(COMPOSE, &s).is_err());
        }
        let mut s = base;
        s[0].name = "nope".into();
        assert!(matches!(
            apply(COMPOSE, &s),
            Err(ConfigError::UnknownService(_))
        ));
    }

    #[test]
    fn split_image_handles_registry_ports() {
        assert_eq!(
            split_image("localhost:5000/app"),
            ("localhost:5000/app".into(), "latest".into())
        );
        assert_eq!(
            split_image("localhost:5000/app:v1"),
            ("localhost:5000/app".into(), "v1".into())
        );
        assert_eq!(split_image("nginx"), ("nginx".into(), "latest".into()));
    }

    #[test]
    fn raw_mode_requires_services() {
        assert!(
            resolve(
                COMPOSE,
                Some(&AppConfig::Raw {
                    compose: "foo: 1".into()
                })
            )
            .is_err()
        );
        assert!(
            resolve(
                COMPOSE,
                Some(&AppConfig::Raw {
                    compose: COMPOSE.into()
                })
            )
            .is_ok()
        );
        assert_eq!(resolve(COMPOSE, None).expect("ok"), COMPOSE);
    }
}
