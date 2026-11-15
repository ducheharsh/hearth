//! Connectivity: which interface the Mac uses, Wi-Fi power, and whether the internet
//! is reachable. Nothing here needs administrator rights.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::{CommandError, run};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Link {
    /// BSD name, e.g. `en0`.
    pub device: String,
    /// `wifi` | `ethernet` | `other`
    pub kind: &'static str,
    /// Hardware port name, e.g. `Wi-Fi`, `USB 10/100/1000 LAN`.
    pub port: String,
    pub ip: Option<String>,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NetworkStatus {
    /// The interface the default route uses (`None` = no network at all).
    pub primary: Option<Link>,
    /// The Wi-Fi device, and whether its radio is on.
    pub wifi_device: Option<String>,
    pub wifi_on: Option<bool>,
    /// A wired port exists (built-in or an adapter).
    pub has_ethernet: bool,
}

/// `networksetup -listallhardwareports` → (port name, device).
#[must_use]
pub fn parse_ports(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut port = None;
    for line in raw.lines() {
        if let Some(p) = line.strip_prefix("Hardware Port: ") {
            port = Some(p.trim().to_owned());
        } else if let (Some(d), Some(p)) = (line.strip_prefix("Device: "), port.take()) {
            out.push((p, d.trim().to_owned()));
        }
    }
    out
}

fn kind(port: &str) -> &'static str {
    let p = port.to_ascii_lowercase();
    if p.contains("wi-fi") || p.contains("airport") {
        "wifi"
    } else if p.contains("ethernet") || p.contains("lan") {
        "ethernet"
    } else {
        "other"
    }
}

/// `route -n get default` → (interface, gateway).
#[must_use]
pub fn parse_route(raw: &str) -> (Option<String>, Option<String>) {
    let field = |name: &str| {
        raw.lines()
            .find_map(|l| l.trim().strip_prefix(name))
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    (field("interface:"), field("gateway:"))
}

pub async fn status() -> Result<NetworkStatus, CommandError> {
    let ports = parse_ports(&run("networksetup", &["-listallhardwareports"], &[]).await?);
    // No default route (offline) makes `route` fail; that's a state, not an error.
    let (iface, gateway) = run("route", &["-n", "get", "default"], &[])
        .await
        .map(|r| parse_route(&r))
        .unwrap_or_default();
    let wifi_device = ports
        .iter()
        .find(|(p, _)| kind(p) == "wifi")
        .map(|(_, d)| d.clone());
    let wifi_on = match &wifi_device {
        Some(d) => run("networksetup", &["-getairportpower", d], &[])
            .await
            .ok()
            .map(|s| s.trim_end().ends_with(": On")),
        None => None,
    };
    let primary = match iface {
        Some(device) => {
            let port = ports
                .iter()
                .find(|(_, d)| *d == device)
                .map_or_else(|| device.clone(), |(p, _)| p.clone());
            let ip = run("ipconfig", &["getifaddr", &device], &[])
                .await
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());
            Some(Link {
                kind: kind(&port),
                device,
                port,
                ip,
                gateway,
            })
        }
        None => None,
    };
    Ok(NetworkStatus {
        primary,
        wifi_device,
        wifi_on,
        has_ethernet: ports.iter().any(|(p, _)| kind(p) == "ethernet"),
    })
}

/// Turn the Wi-Fi radio on or off (no administrator rights needed).
pub async fn set_wifi(device: &str, on: bool) -> Result<(), CommandError> {
    run(
        "networksetup",
        &["-setairportpower", device, if on { "on" } else { "off" }],
        &[],
    )
    .await
    .map(|_| ())
}

/// Can we reach the internet? Tries a few well-known anycast addresses over TCP 443 and
/// returns the fastest round trip, or `None` when none answer within `timeout`.
pub async fn probe(timeout: Duration) -> Option<Duration> {
    const TARGETS: [&str; 3] = ["1.1.1.1:443", "8.8.8.8:443", "9.9.9.9:443"];
    let attempts = TARGETS.iter().map(|t| async move {
        let start = Instant::now();
        tokio::time::timeout(timeout, tokio::net::TcpStream::connect(t))
            .await
            .ok()?
            .ok()
            .map(|_| start.elapsed())
    });
    fastest(futures_util::future::join_all(attempts).await)
}

fn fastest(results: Vec<Option<Duration>>) -> Option<Duration> {
    results.into_iter().flatten().min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ports_and_kinds() {
        let raw = "\nHardware Port: USB 10/100/1000 LAN\nDevice: en7\nEthernet Address: x\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: y\n\nHardware Port: Thunderbolt Bridge\nDevice: bridge0\n";
        let p = parse_ports(raw);
        assert_eq!(p.len(), 3);
        assert_eq!(kind(&p[0].0), "ethernet");
        assert_eq!(kind(&p[1].0), "wifi");
        assert_eq!(kind(&p[2].0), "other");
        assert_eq!(kind("Ethernet Adapter (en3)"), "ethernet");
    }

    #[test]
    fn parses_default_route() {
        let raw = "   route to: default\ndestination: default\n       mask: default\n    gateway: 10.40.0.1\n  interface: en0\n";
        assert_eq!(
            parse_route(raw),
            (Some("en0".into()), Some("10.40.0.1".into()))
        );
        assert_eq!(parse_route(""), (None, None));
    }

    #[test]
    fn fastest_probe_wins() {
        let ms = Duration::from_millis;
        assert_eq!(
            fastest(vec![None, Some(ms(30)), Some(ms(12))]),
            Some(ms(12))
        );
        assert_eq!(fastest(vec![None, None]), None);
    }
}
