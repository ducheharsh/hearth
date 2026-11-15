//! Signed-in devices.
//!
//! Authelia keeps its sessions in the gateway's Redis (`authelia-session:<cookie>`), which is
//! the source of truth for who is signed in; deleting a key signs that device out at once.
//! Redis values are encrypted, so *who* and *what device* come from Caddy: it streams JSON
//! access logs to [`run_receiver`] with the signed-in user (set only by Authelia; Caddy strips
//! client-sent identity headers), the session cookie and the browser. hearthd keeps only a
//! SHA-256 of the cookie, never the cookie itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncBufReadExt as _, BufReader};

use crate::state::{AppState, SharedState};
use crate::store;

const KEY_PREFIX: &str = "authelia-session:";
/// Authelia's session cookie.
pub const COOKIE: &str = "authelia_session";

/// What hearth has seen of one session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Seen {
    pub user: Option<String>,
    pub agent: String,
    /// Tailscale address it came from, when visible.
    pub ip: Option<String>,
    pub first: u64,
    pub last: u64,
    /// Public ports of the sites it opened after signing in.
    pub ports: BTreeSet<u16>,
}

/// Sessions by cookie hash.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub seen: BTreeMap<String, Seen>,
    #[serde(skip)]
    dirty: bool,
}

/// Stable, non-reversible id for a session cookie.
#[must_use]
pub fn hash(sid: &str) -> String {
    Sha256::digest(sid.as_bytes())
        .iter()
        .take(16)
        .fold(String::with_capacity(32), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// One access-log line from Caddy, reduced to what matters here.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub sid: String,
    pub user: Option<String>,
    pub agent: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub ok: bool,
    /// Went through the login check.
    pub gate: bool,
    /// Authelia let it in (only then is `user` set).
    pub authed: bool,
    pub at: u64,
}

/// Tailscale's CGNAT range, 100.64.0.0/10.
fn tailnet_ip(ip: &str) -> bool {
    let mut parts = ip.split('.');
    parts.next() == Some("100")
        && parts
            .next()
            .and_then(|p| p.parse::<u8>().ok())
            .is_some_and(|b| (64..128).contains(&b))
}

#[must_use]
pub fn parse_hit(line: &str) -> Option<Hit> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let sid = v["sid"].as_str().filter(|s| !s.is_empty())?.to_owned();
    let req = &v["request"];
    let header = |k: &str| req["headers"][k][0].as_str().unwrap_or_default().to_owned();
    let forwarded = header("X-Forwarded-For");
    let ip = std::iter::once(req["remote_ip"].as_str().unwrap_or_default())
        .chain(forwarded.split(',').map(str::trim))
        .find(|ip| tailnet_ip(ip))
        .map(str::to_owned);
    let port = req["host"]
        .as_str()
        .and_then(|h| h.rsplit_once(':'))
        .and_then(|(_, p)| p.parse().ok());
    let status = v["status"].as_u64().unwrap_or(0);
    // Caddy types `log_append` values: `1` arrives as a number.
    let flag = |k: &str| v[k].as_u64() == Some(1) || v[k].as_str() == Some("1");
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a Unix time in seconds
    let at = v["ts"].as_f64().unwrap_or(0.0) as u64;
    Some(Hit {
        sid,
        user: v["user"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        agent: header("User-Agent").chars().take(300).collect(),
        ip,
        port,
        ok: (200..400).contains(&status),
        gate: flag("gate"),
        authed: flag("authed"),
        at,
    })
}

impl Store {
    pub fn record(&mut self, h: Hit) {
        let e = self.seen.entry(hash(&h.sid)).or_insert_with(|| Seen {
            first: h.at,
            ..Seen::default()
        });
        if h.authed && h.user.is_some() {
            e.user = h.user;
            if h.ok
                && let Some(p) = h.port
            {
                e.ports.insert(p);
            }
        } else if h.gate {
            // The login turned it away: this session isn't signed in (any more).
            e.user = None;
            e.ports.clear();
        }
        if !h.agent.is_empty() {
            e.agent = h.agent;
        }
        if h.ip.is_some() {
            e.ip = h.ip;
        }
        e.last = e.last.max(h.at);
        self.dirty = true;
    }

    /// Forget sessions Authelia no longer has (signed out or expired).
    pub fn retain(&mut self, active: &BTreeSet<String>) {
        let before = self.seen.len();
        self.seen.retain(|id, _| active.contains(id));
        self.dirty |= self.seen.len() != before;
    }
}

/// "Safari on iPhone", from a User-Agent.
#[must_use]
pub fn describe(agent: &str) -> String {
    let has = |s: &str| agent.contains(s);
    let os = if has("iPhone") {
        Some("iPhone")
    } else if has("iPad") {
        Some("iPad")
    } else if has("Android") {
        Some("Android")
    } else if has("CrOS") {
        Some("ChromeOS")
    } else if has("Mac OS X") || has("Macintosh") {
        Some("Mac")
    } else if has("Windows") {
        Some("Windows")
    } else if has("Linux") {
        Some("Linux")
    } else {
        None
    };
    let browser = if has("Edg/") || has("EdgiOS") {
        Some("Edge")
    } else if has("OPR/") {
        Some("Opera")
    } else if has("Firefox/") || has("FxiOS") {
        Some("Firefox")
    } else if has("CriOS") || has("Chrome/") {
        Some("Chrome")
    } else if has("Safari/") {
        Some("Safari")
    } else if agent.starts_with("curl/") {
        Some("curl")
    } else {
        None
    };
    match (browser, os) {
        (Some(b), Some(o)) => format!("{b} on {o}"),
        (Some(x), None) | (None, Some(x)) => x.to_owned(),
        (None, None) => "Unknown device".into(),
    }
}

/// Where Caddy sends access logs: the daemon's port + 1, on loopback.
#[must_use]
pub fn log_addr(st: &AppState) -> SocketAddr {
    let l = st.config.listen;
    SocketAddr::new(l.ip(), l.port().wrapping_add(1))
}

/// Accept Caddy's log stream (one JSON object per line) and record sessions.
pub async fn run_receiver(st: SharedState) {
    let addr = log_addr(&st);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(%addr, error = %e, "can't receive gateway logs; signed-in devices won't be named");
            return;
        }
    };
    loop {
        let Ok((sock, _)) = listener.accept().await else {
            continue;
        };
        let st = st.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(sock).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(hit) = parse_hit(&line) {
                    st.sessions.write().await.record(hit);
                }
            }
        });
    }
}

fn path(st: &AppState) -> std::path::PathBuf {
    st.config.state_dir.join("sessions.json")
}

pub fn load(st_dir: &std::path::Path) -> Store {
    store::load(&st_dir.join("sessions.json")).unwrap_or_default()
}

/// Save what changed every 15 s.
pub async fn run_saver(st: SharedState) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
    loop {
        tick.tick().await;
        let mut s = st.sessions.write().await;
        if s.dirty {
            if let Err(e) = store::save(&path(&st), &*s) {
                tracing::warn!(error = %e, "saving sessions failed");
            }
            s.dirty = false;
        }
    }
}

/// Live session keys in Redis, by cookie hash.
pub async fn active(st: &AppState) -> anyhow::Result<BTreeMap<String, String>> {
    let out =
        crate::gateway::redis(st, &["--scan", "--pattern", &format!("{KEY_PREFIX}*")]).await?;
    Ok(out
        .lines()
        .filter_map(|k| k.trim().strip_prefix(KEY_PREFIX))
        .map(|sid| (hash(sid), format!("{KEY_PREFIX}{sid}")))
        .collect())
}

/// Sign out the given sessions (by hash) at once. Returns how many were signed out.
pub async fn sign_out(st: &AppState, ids: &BTreeSet<String>) -> anyhow::Result<usize> {
    let live = active(st).await?;
    let keys: Vec<&str> = live
        .iter()
        .filter(|(h, _)| ids.contains(*h))
        .map(|(_, k)| k.as_str())
        .collect();
    if keys.is_empty() {
        return Ok(0);
    }
    let mut args = vec!["DEL"];
    args.extend(&keys);
    crate::gateway::redis(st, &args).await?;
    let mut s = st.sessions.write().await;
    for id in ids {
        s.seen.remove(id);
    }
    s.dirty = true;
    Ok(keys.len())
}

/// Sign out every device of `user` (after disabling, deleting or a password reset).
pub async fn sign_out_user(st: &AppState, user: &str) -> anyhow::Result<usize> {
    if !st.settings.read().await.gateway_enabled {
        return Ok(0);
    }
    let ids: BTreeSet<String> = st
        .sessions
        .read()
        .await
        .seen
        .iter()
        .filter(|(_, s)| s.user.as_deref() == Some(user))
        .map(|(id, _)| id.clone())
        .collect();
    sign_out(st, &ids).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"level":"info","ts":1794737379.5,"logger":"http.log.access.log0","msg":"handled request","request":{"remote_ip":"192.168.117.1","client_ip":"192.168.117.1","proto":"HTTP/2.0","method":"GET","host":"m1.tail.ts.net:9443","uri":"/","headers":{"User-Agent":["Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1"],"X-Forwarded-For":["100.101.5.7"],"Cookie":["REDACTED"]}},"status":200,"user":"mom","sid":"abc*DEF","gate":1,"authed":1}"#;

    #[test]
    fn parses_caddy_access_lines() {
        let h = parse_hit(LINE).expect("hit");
        assert_eq!(h.user.as_deref(), Some("mom"));
        assert_eq!(h.port, Some(9443));
        assert_eq!(h.ip.as_deref(), Some("100.101.5.7"), "tailnet IP from XFF");
        assert!(h.ok);
        assert_eq!(h.at, 1_794_737_379);
        assert!(
            parse_hit(&LINE.replace("abc*DEF", "")).is_none(),
            "no session, no hit"
        );
        assert!(parse_hit("not json").is_none());
    }

    #[test]
    fn records_by_hash_and_never_keeps_the_cookie() {
        let mut s = Store::default();
        s.record(parse_hit(LINE).expect("hit"));
        let json = serde_json::to_string(&s).expect("ser");
        assert!(!json.contains("abc*DEF"));
        let seen = &s.seen[&hash("abc*DEF")];
        assert_eq!(seen.ports, BTreeSet::from([9443]));
        // A later anonymous hit (the login portal) keeps the user.
        let portal = LINE
            .replace(r#""user":"mom""#, r#""user":"""#)
            .replace(r#""gate":1,"authed":1"#, r#""gate":"","authed":"""#);
        s.record(parse_hit(&portal).expect("hit"));
        assert_eq!(s.seen[&hash("abc*DEF")].user.as_deref(), Some("mom"));
        // Turned away by the login (signed out elsewhere): no longer signed in.
        s.record(
            parse_hit(
                &LINE
                    .replace(r#""authed":1"#, r#""authed":"""#)
                    .replace("200", "302"),
            )
            .expect("hit"),
        );
        assert_eq!(s.seen[&hash("abc*DEF")].user, None);
        assert!(s.seen[&hash("abc*DEF")].ports.is_empty());
        s.retain(&BTreeSet::new());
        assert!(s.seen.is_empty());
    }

    #[test]
    fn describes_devices() {
        let h = parse_hit(LINE).expect("hit");
        assert_eq!(describe(&h.agent), "Safari on iPhone");
        assert_eq!(
            describe(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/130.0 Safari/537.36"
            ),
            "Chrome on Mac"
        );
        assert_eq!(
            describe("Mozilla/5.0 (Windows NT 10.0) Chrome/130.0 Safari/537.36 Edg/130.0"),
            "Edge on Windows"
        );
        assert_eq!(describe(""), "Unknown device");
        assert!(tailnet_ip("100.127.1.1") && !tailnet_ip("100.128.0.1") && !tailnet_ip("10.0.0.1"));
    }
}
