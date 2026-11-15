//! Keeping the Mac running as a server: stay awake on power, watch the internet,
//! and repair what can be repaired without a person (Wi-Fi radio, Tailscale).

use std::collections::VecDeque;
use std::time::Duration;

use serde::Serialize;

use crate::events::{Level, now};
use crate::state::SharedState;

const CHECK_EVERY: Duration = Duration::from_secs(30);
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
/// Failed probes in a row before we call it an outage (one miss is just noise).
const DOWN_AFTER: u32 = 2;
const KEEP_OUTAGES: usize = 20;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Outage {
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Debug, Default, Serialize)]
pub struct Connectivity {
    /// `None` until the first check.
    pub online: Option<bool>,
    pub latency_ms: Option<u64>,
    pub checked_at: Option<u64>,
    pub outages: VecDeque<Outage>,
    #[serde(skip)]
    misses: u32,
    /// Tailscale was connected when the current outage began.
    #[serde(skip)]
    tailscale_was_up: bool,
}

impl Connectivity {
    /// Record one probe result at time `t`; returns what changed, if anything.
    fn step(&mut self, rtt: Option<Duration>, ts_up: bool, t: u64) -> Option<Change> {
        self.checked_at = Some(t);
        self.latency_ms = rtt.map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        if rtt.is_some() {
            self.misses = 0;
            let was_down = self.online == Some(false);
            self.online = Some(true);
            if !was_down {
                return None;
            }
            let start = self.outages.back().map_or(t, |o| o.start);
            if let Some(o) = self.outages.back_mut() {
                o.end = Some(t);
            }
            return Some(Change::Up {
                minutes: t.saturating_sub(start).div_ceil(60),
                reconnect: self.tailscale_was_up && !ts_up,
            });
        }
        self.misses += 1;
        if self.misses < DOWN_AFTER || self.online == Some(false) {
            return None;
        }
        self.online = Some(false);
        self.tailscale_was_up = ts_up;
        if self.outages.len() == KEEP_OUTAGES {
            self.outages.pop_front();
        }
        // The outage began at the first missed check.
        self.outages.push_back(Outage {
            start: t.saturating_sub(CHECK_EVERY.as_secs() * u64::from(DOWN_AFTER - 1)),
            end: None,
        });
        Some(Change::Down)
    }
}

#[derive(Debug, Default)]
pub struct AlwaysOnState {
    pub conn: Connectivity,
    caffeinate: Option<tokio::process::Child>,
}

impl AlwaysOnState {
    /// The sleep assertion is held right now.
    pub fn holding(&mut self) -> bool {
        self.caffeinate
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }
}

/// Hold (or release) the sleep assertion to match the setting; restart it if it died.
pub async fn run_keeper(st: SharedState) {
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    loop {
        tick.tick().await;
        let want = st.settings.read().await.always_on.keep_awake;
        let mut a = st.always_on.lock().await;
        let held = a.holding();
        if want && !held {
            match hearth_host::power::caffeinate(std::process::id()) {
                Ok(child) => a.caffeinate = Some(child),
                Err(e) => tracing::warn!(error = %e, "could not start caffeinate"),
            }
        } else if !want && let Some(mut c) = a.caffeinate.take() {
            let _ = c.kill().await;
        }
    }
}

/// Check the internet on a timer, raise outages, and repair what we can.
pub async fn run_watch(st: SharedState) {
    let mut tick = tokio::time::interval(CHECK_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let cfg = st.settings.read().await.always_on;
        if !cfg.watch_connectivity {
            continue;
        }
        check(&st, cfg.auto_repair).await;
    }
}

/// One check: probe, update state, and report changes.
pub async fn check(st: &SharedState, repair: bool) {
    let rtt = hearth_host::network::probe(PROBE_TIMEOUT).await;
    let ts_up = st.tailscale.read().await.backend_state == "Running";
    let t = now();
    let change = st.always_on.lock().await.conn.step(rtt, ts_up, t);
    match change {
        Some(Change::Down) => {
            st.event(
                Level::Warning,
                "network",
                "Internet is down",
                "hearth keeps checking and will tell you when it's back.",
            )
            .await;
            if repair {
                repair_wifi(st).await;
            }
        }
        Some(Change::Up { minutes, reconnect }) => {
            st.event(
                Level::Success,
                "network",
                "Back online",
                format!("The internet was down for about {minutes} min."),
            )
            .await;
            if repair && reconnect {
                match hearth_host::tailscale::set_up(true).await {
                    Ok(()) => {
                        st.event(
                            Level::Info,
                            "network",
                            "Tailscale reconnected",
                            "It had dropped during the outage.",
                        )
                        .await;
                    }
                    Err(e) => {
                        st.event(
                            Level::Warning,
                            "network",
                            "Tailscale didn't reconnect",
                            e.to_string(),
                        )
                        .await;
                    }
                }
            }
        }
        None => {
            // Still down: keep trying the radio, in case something turned it off again.
            if repair && st.always_on.lock().await.conn.online == Some(false) {
                repair_wifi(st).await;
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Change {
    Down,
    Up { minutes: u64, reconnect: bool },
}

/// If the Wi-Fi radio is off and nothing else carries the network, turn it back on.
async fn repair_wifi(st: &SharedState) {
    let Ok(net) = hearth_host::network::status().await else {
        return;
    };
    if net.primary.is_some() || net.wifi_on != Some(false) {
        return;
    }
    let Some(dev) = net.wifi_device else { return };
    match hearth_host::network::set_wifi(&dev, true).await {
        Ok(()) => {
            st.event(
                Level::Info,
                "network",
                "Wi-Fi turned back on",
                "It was off, so this Mac had no network.",
            )
            .await;
        }
        Err(e) => tracing::warn!(error = %e, "could not turn Wi-Fi on"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: Option<Duration> = Some(Duration::from_millis(9));

    #[test]
    fn one_miss_is_noise_two_is_an_outage() {
        let mut c = Connectivity::default();
        assert_eq!(c.step(UP, true, 0), None);
        assert_eq!(c.step(None, true, 30), None, "one miss");
        assert_eq!(c.step(UP, true, 60), None, "recovered before it counted");
        assert_eq!(c.step(None, true, 90), None);
        assert_eq!(c.step(None, true, 120), Some(Change::Down));
        assert_eq!(c.step(None, true, 150), None, "reported once");
        assert_eq!(
            c.outages.back().map(|o| o.start),
            Some(90),
            "starts at the first miss"
        );
    }

    #[test]
    fn recovery_reports_duration_and_tailscale() {
        let mut c = Connectivity::default();
        c.step(None, true, 0);
        c.step(None, true, 30);
        // Back after ~7 minutes; Tailscale didn't come back by itself.
        assert_eq!(
            c.step(UP, false, 420),
            Some(Change::Up {
                minutes: 7,
                reconnect: true
            })
        );
        assert_eq!(c.outages.back().and_then(|o| o.end), Some(420));
        // Tailscale was already off before the outage: leave it alone.
        c.step(None, false, 500);
        c.step(None, false, 530);
        assert_eq!(
            c.step(UP, false, 560),
            Some(Change::Up {
                minutes: 1,
                reconnect: false
            })
        );
    }

    #[test]
    fn keeps_a_bounded_history() {
        let mut c = Connectivity::default();
        for i in 0..30u64 {
            c.step(None, true, i * 100);
            c.step(None, true, i * 100 + 30);
            c.step(UP, true, i * 100 + 60);
        }
        assert_eq!(c.outages.len(), KEEP_OUTAGES);
    }
}
