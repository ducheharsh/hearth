//! Alerts: forward important events to your phone (ntfy, Telegram, webhook), plus
//! monitors that create those events (app crashes, low disk, memory pressure,
//! failed logins).

use std::collections::BTreeMap;
use std::time::Duration;

use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::events::{Event, Level, now};
use crate::state::SharedState;
use crate::store;

const DEDUPE_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MinLevel {
    Info,
    #[default]
    Warning,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Ntfy {
    pub enabled: bool,
    /// e.g. `https://ntfy.sh` or your own server.
    pub server: String,
    pub topic: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Telegram {
    pub enabled: bool,
    pub chat_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Webhook {
    pub enabled: bool,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AlertSettings {
    pub min_level: MinLevel,
    pub ntfy: Ntfy,
    pub telegram: Telegram,
    pub webhook: Webhook,
    pub watch_apps: bool,
    pub watch_resources: bool,
    pub watch_logins: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AlertSecrets {
    pub ntfy_token: Option<String>,
    pub telegram_bot_token: Option<String>,
}

pub fn load_settings(state_dir: &std::path::Path) -> AlertSettings {
    let path = state_dir.join("alerts.json");
    let fresh = !path.exists();
    let mut s: AlertSettings = store::load(&path).unwrap_or_default();
    if fresh {
        s.watch_apps = true;
        s.watch_resources = true;
        s.watch_logins = true;
    }
    if s.ntfy.server.is_empty() {
        s.ntfy.server = "https://ntfy.sh".into();
    }
    s
}

pub fn load_secrets(st: &SharedState) -> AlertSecrets {
    store::load(&st.config.secrets_dir().join("alerts.json")).unwrap_or_default()
}

fn level_ok(min: MinLevel, l: Level) -> bool {
    let rank = match l {
        Level::Info | Level::Success => MinLevel::Info,
        Level::Warning => MinLevel::Warning,
        Level::Error => MinLevel::Error,
    };
    rank >= min
}

/// One outgoing request per enabled channel (pure: easy to test).
#[derive(Debug, PartialEq)]
pub struct Outgoing {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

#[derive(Debug, PartialEq)]
pub enum Body {
    Text(String),
    Json(serde_json::Value),
}

#[must_use]
pub fn build(
    settings: &AlertSettings,
    secrets: &AlertSecrets,
    server_name: &str,
    e: &Event,
) -> Vec<Outgoing> {
    let mut out = Vec::new();
    let title = format!("{server_name}: {}", e.title);
    if settings.ntfy.enabled && !settings.ntfy.topic.is_empty() {
        let mut headers = vec![
            ("Title".to_owned(), title.clone()),
            (
                "Tags".to_owned(),
                match e.level {
                    Level::Error => "rotating_light",
                    Level::Warning => "warning",
                    _ => "fire",
                }
                .to_owned(),
            ),
            (
                "Priority".to_owned(),
                match e.level {
                    Level::Error => "high",
                    Level::Warning => "default",
                    _ => "low",
                }
                .to_owned(),
            ),
        ];
        if let Some(t) = &secrets.ntfy_token {
            headers.push(("Authorization".to_owned(), format!("Bearer {t}")));
        }
        out.push(Outgoing {
            url: format!(
                "{}/{}",
                settings.ntfy.server.trim_end_matches('/'),
                settings.ntfy.topic
            ),
            headers,
            body: Body::Text(if e.message.is_empty() {
                e.title.clone()
            } else {
                e.message.clone()
            }),
        });
    }
    if settings.telegram.enabled
        && let Some(token) = &secrets.telegram_bot_token
    {
        out.push(Outgoing {
            url: format!("https://api.telegram.org/bot{token}/sendMessage"),
            headers: vec![],
            body: Body::Json(json!({ "chat_id": settings.telegram.chat_id, "text": format!("🔥 {title}\n{}", e.message), "disable_web_page_preview": true })),
        });
    }
    if settings.webhook.enabled && !settings.webhook.url.is_empty() {
        out.push(Outgoing {
            url: settings.webhook.url.clone(),
            headers: vec![],
            body: Body::Json(json!({ "server": server_name, "level": e.level, "source": e.source, "title": e.title, "message": e.message, "at": e.at })),
        });
    }
    out
}

pub async fn send(out: Vec<Outgoing>) -> Vec<String> {
    let Ok(client) = reqwest::Client::builder()
        .user_agent("hearth")
        .timeout(Duration::from_secs(10))
        .build()
    else {
        return vec!["http client unavailable".into()];
    };
    let mut errors = Vec::new();
    for o in out {
        let mut req = client.post(&o.url);
        for (k, v) in &o.headers {
            req = req.header(k, v);
        }
        req = match o.body {
            Body::Text(t) => req.body(t),
            Body::Json(j) => req.json(&j),
        };
        match req.send().await {
            Ok(r) if r.status().is_success() => {}
            Ok(r) => errors.push(format!("{} → HTTP {}", host_of(&o.url), r.status())),
            Err(e) => errors.push(format!("{} → {e}", host_of(&o.url))),
        }
    }
    errors
}

fn host_of(url: &str) -> String {
    url.split('/').nth(2).unwrap_or(url).to_owned()
}

/// Forward events to channels (filtered + de-duplicated).
pub async fn run_dispatcher(st: SharedState, mut rx: UnboundedReceiver<Event>) {
    let mut recent: BTreeMap<String, u64> = BTreeMap::new();
    while let Some(e) = rx.recv().await {
        if e.source == "alerts" {
            continue; // never alert about alerting
        }
        let settings = st.alerts.read().await.clone();
        if !level_ok(settings.min_level, e.level) {
            continue;
        }
        let key = format!("{}|{}", e.title, e.message);
        let t = now();
        recent.retain(|_, at| t - *at < DEDUPE_SECS);
        if recent.contains_key(&key) {
            continue;
        }
        recent.insert(key, t);
        let name = st.settings.read().await.display_name.clone();
        let out = build(&settings, &load_secrets(&st), &name, &e);
        if out.is_empty() {
            continue;
        }
        let errors = send(out).await;
        if !errors.is_empty() {
            tracing::warn!(?errors, "alert delivery failed");
        }
    }
}

/// App crash / health, low disk and memory pressure monitors.
#[allow(clippy::too_many_lines)] // one loop with three small, independent watchers
pub async fn run_monitors(st: SharedState) {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    let mut down: BTreeMap<String, bool> = BTreeMap::new();
    let mut disk_warned = 0u64;
    let mut swap_high_since: Option<u64> = None;
    let mut swap_warned = 0u64;
    loop {
        tick.tick().await;
        let settings = st.alerts.read().await.clone();
        if settings.watch_apps
            && let Some(docker) = &st.docker
            && let Ok(list) = docker.containers().await
        {
            let names: BTreeMap<String, String> = st
                .catalog
                .read()
                .await
                .iter()
                .map(|a| (a.project_name(), a.name.clone()))
                .collect();
            let mut now_down: BTreeMap<String, bool> = BTreeMap::new();
            for c in list
                .iter()
                .filter(|c| c.project.as_deref().is_some_and(|p| names.contains_key(p)))
            {
                let Ok(d) = docker.inspect(&c.id).await else {
                    continue;
                };
                let crashed = d.status == "restarting"
                    || d.health.as_deref() == Some("unhealthy")
                    || (!d.running
                        && d.status == "exited"
                        && !matches!(d.exit_code, Some(0 | 137 | 143)));
                let p = c.project.clone().unwrap_or_default();
                *now_down.entry(p).or_default() |= crashed;
            }
            for (p, is_down) in &now_down {
                let was = down.get(p).copied().unwrap_or(false);
                let name = names.get(p).cloned().unwrap_or_else(|| p.clone());
                if *is_down && !was {
                    st.event(
                        Level::Error,
                        "monitor",
                        format!("{name} is down"),
                        "It crashed or is failing its health check. Open Containers for logs.",
                    )
                    .await;
                } else if !*is_down && was {
                    st.event(Level::Success, "monitor", format!("{name} recovered"), "")
                        .await;
                }
            }
            down = now_down;
        }
        if settings.watch_resources
            && let Some(s) = st.metrics.read().await.latest.clone()
        {
            let t = now();
            let reserve = st.settings.read().await.capacity.policy().disk_reserve;
            if let Some(root) = s
                .disks
                .iter()
                .find(|d| d.mount == std::path::Path::new("/"))
                && root.free < reserve
                && t - disk_warned > 6 * 3600
            {
                disk_warned = t;
                st.event(
                    Level::Warning,
                    "monitor",
                    "Internal disk almost full",
                    format!(
                        "{} GB free. Installs are blocked until space is freed.",
                        root.free / (1 << 30)
                    ),
                )
                .await;
            }
            let swap_pct = (s.swap_used * 100).checked_div(s.swap_total).unwrap_or(0);
            if swap_pct > 80 {
                let since = *swap_high_since.get_or_insert(t);
                if t - since > 15 * 60 && t - swap_warned > 6 * 3600 {
                    swap_warned = t;
                    st.event(
                        Level::Warning,
                        "monitor",
                        "Memory under pressure",
                        "Heavy swapping for 15 minutes. Stop an app you don't need.",
                    )
                    .await;
                }
            } else {
                swap_high_since = None;
            }
        }
    }
}

/// Parse a failed sign-in from an Authelia log line: `(user, remote_ip)`.
#[must_use]
pub fn failed_login(line: &str) -> Option<(String, String)> {
    if !line.contains("Unsuccessful") || !line.contains("authentication attempt") {
        return None;
    }
    let field = |k: &str| {
        let i = line.find(&format!("{k}="))? + k.len() + 1;
        let rest = &line[i..];
        let v = if let Some(r) = rest.strip_prefix('"') {
            r.split('"').next()
        } else {
            rest.split_whitespace().next()
        };
        v.map(str::to_owned)
    };
    let user = field("username").or_else(|| {
        let i = line.find("by user '")? + 9;
        line[i..].split('\'').next().map(str::to_owned)
    })?;
    Some((user, field("remote_ip").unwrap_or_else(|| "unknown".into())))
}

/// Follow Authelia's log for failed sign-ins (while the gateway runs).
pub async fn run_login_watch(st: SharedState) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        if !st.alerts.read().await.watch_logins || !st.settings.read().await.gateway_enabled {
            continue;
        }
        let Some(docker) = st.docker.clone() else {
            continue;
        };
        let Ok(list) = docker.containers().await else {
            continue;
        };
        let Some(c) = list.iter().find(|c| {
            c.project.as_deref() == Some("hearth-gateway")
                && c.name.contains("authelia")
                && c.state == "running"
        }) else {
            continue;
        };
        let mut logs = docker.logs(&c.id, 0, true);
        while let Some(Ok(line)) = logs.next().await {
            if let Some((user, ip)) = failed_login(&line.text) {
                st.event(
                    Level::Warning,
                    "security",
                    "Failed sign-in",
                    format!("User “{user}” from {ip}."),
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(level: Level) -> Event {
        Event {
            seq: 1,
            at: 0,
            level,
            source: "monitor",
            title: "Immich is down".into(),
            message: "It crashed.".into(),
        }
    }

    #[test]
    fn builds_requests_per_channel() {
        let s = AlertSettings {
            ntfy: Ntfy {
                enabled: true,
                server: "https://ntfy.sh/".into(),
                topic: "hearth-x".into(),
            },
            telegram: Telegram {
                enabled: true,
                chat_id: "42".into(),
            },
            webhook: Webhook {
                enabled: true,
                url: "http://127.0.0.1:9/hook".into(),
            },
            ..AlertSettings::default()
        };
        let secrets = AlertSecrets {
            ntfy_token: Some("tk".into()),
            telegram_bot_token: Some("123:abc".into()),
        };
        let out = build(&s, &secrets, "m1", &ev(Level::Error));
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].url, "https://ntfy.sh/hearth-x");
        assert!(out[0].headers.contains(&("Priority".into(), "high".into())));
        assert!(
            out[0]
                .headers
                .contains(&("Authorization".into(), "Bearer tk".into()))
        );
        assert_eq!(
            out[1].url,
            "https://api.telegram.org/bot123:abc/sendMessage"
        );
        assert!(matches!(&out[2].body, Body::Json(j) if j["title"] == "Immich is down"));
    }

    #[test]
    fn telegram_needs_token_and_disabled_channels_are_skipped() {
        let mut s = AlertSettings::default();
        s.telegram.enabled = true;
        assert_eq!(
            build(&s, &AlertSecrets::default(), "m1", &ev(Level::Error)).len(),
            0
        );
    }

    #[test]
    fn levels() {
        assert!(level_ok(MinLevel::Warning, Level::Error));
        assert!(!level_ok(MinLevel::Warning, Level::Success));
        assert!(level_ok(MinLevel::Info, Level::Info));
    }

    #[test]
    fn parses_authelia_failed_login() {
        let l = r#"time="2026-11-15T03:00:00Z" level=error msg="Unsuccessful 1FA authentication attempt by user 'harsh'" error="invalid credentials" method=POST path=/api/firstfactor remote_ip=100.64.0.7"#;
        assert_eq!(failed_login(l), Some(("harsh".into(), "100.64.0.7".into())));
        assert!(failed_login("level=info msg=\"Successful 1FA authentication attempt\"").is_none());
    }
}
