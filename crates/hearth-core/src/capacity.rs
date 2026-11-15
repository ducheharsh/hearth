//! Capacity planning: "can this machine take one more app right now?"
//!
//! The planner combines *declared* needs (manifest `resources`) with *observed*
//! usage (a [`HostSnapshot`]) and a [`CapacityPolicy`]. It never measures
//! anything itself, which keeps it deterministic and testable.

use serde::Serialize;

use crate::manifest::AppManifest;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Point-in-time view of the host, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HostSnapshot {
    pub mem_total: u64,
    /// Memory the OS reports as available (free + reclaimable).
    pub mem_available: u64,
    /// Free space on the internal disk.
    pub disk_free: u64,
    /// Free space on the vault, or `None` if it is not mounted.
    pub vault_free: Option<u64>,
}

/// Tunables. Defaults suit an 8 GB Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CapacityPolicy {
    /// RAM that must stay free for the OS and native apps.
    pub mem_reserve: u64,
    /// Warn (but allow) when free RAM after install would drop below this.
    pub mem_warn_headroom: u64,
    /// Never let the internal disk drop below this.
    pub disk_reserve: u64,
}

impl Default for CapacityPolicy {
    fn default() -> Self {
        Self {
            mem_reserve: 1536 * MIB,
            mem_warn_headroom: 1024 * MIB,
            disk_reserve: 15 * GIB,
        }
    }
}

/// Overall outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    Ok,
    Warn,
    Deny,
}

/// Outcome plus human-readable reasons, ready to show in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verdict {
    pub fit: Fit,
    pub reasons: Vec<String>,
    /// Estimated free RAM (bytes) after the app starts.
    pub mem_after: i64,
}

impl Verdict {
    fn raise(&mut self, level: Fit, reason: String) {
        if level as u8 > self.fit as u8 {
            self.fit = level;
        }
        self.reasons.push(reason);
    }
}

/// Decide whether `app` fits on a host in state `host`.
///
/// ```
/// use hearth_core::{capacity::*, AppManifest};
/// # let app = AppManifest::from_yaml("id: a\nname: a\ndescription: d\ncategory: c\nport: 4000\n\
/// #   upstream: { service: w, port: 80 }\nresources: { ram_mb: 6000 }\n\
/// #   compose: |\n  services:\n    w:\n      image: x\n").unwrap();
/// let gib = 1024 * 1024 * 1024;
/// let host = HostSnapshot { mem_total: 8 * gib, mem_available: 3 * gib, disk_free: 50 * gib, vault_free: None };
/// let v = evaluate(&app, &host, &CapacityPolicy::default());
/// assert_eq!(v.fit, Fit::Deny); // 6 GB app on 3 GB free
/// ```
#[must_use]
pub fn evaluate(app: &AppManifest, host: &HostSnapshot, policy: &CapacityPolicy) -> Verdict {
    let need = app.resources.ram_mb * MIB;
    let mem_after = to_i64(host.mem_available) - to_i64(need);
    let mut v = Verdict {
        fit: Fit::Ok,
        reasons: Vec::new(),
        mem_after,
    };

    // ── Memory ──
    if need > host.mem_total.saturating_sub(policy.mem_reserve) {
        v.raise(
            Fit::Deny,
            format!(
                "needs {} but this machine can never spare more than {}",
                human(need),
                human(host.mem_total.saturating_sub(policy.mem_reserve))
            ),
        );
    } else if mem_after < to_i64(policy.mem_reserve) {
        v.raise(
            Fit::Deny,
            format!(
                "needs {} but only {} is available ({} must stay free); stop something first",
                human(need),
                human(host.mem_available),
                human(policy.mem_reserve)
            ),
        );
    } else if mem_after < to_i64(policy.mem_reserve + policy.mem_warn_headroom) {
        v.raise(
            Fit::Warn,
            format!(
                "fits, but leaves only {} free RAM; expect swapping under load",
                human(u64::try_from(mem_after).unwrap_or(0))
            ),
        );
    }

    // ── Disk ──
    if host.disk_free < policy.disk_reserve {
        v.raise(
            Fit::Deny,
            format!(
                "internal disk has {} free (keep at least {})",
                human(host.disk_free),
                human(policy.disk_reserve)
            ),
        );
    }

    // ── Vault ──
    if app.resources.needs_vault {
        match host.vault_free {
            None => v.raise(
                Fit::Deny,
                "needs the external SSD, which is not mounted".into(),
            ),
            Some(free) if free < app.resources.vault_gb * GIB => v.raise(
                Fit::Warn,
                format!(
                    "Expects ~{} GB of storage but the SSD has {} free",
                    app.resources.vault_gb,
                    human(free)
                ),
            ),
            Some(_) => {}
        }
    }

    if v.reasons.is_empty() {
        v.reasons.push(format!(
            "Fits: ~{} needed, ~{} free afterwards",
            human(need),
            human(u64::try_from(mem_after).unwrap_or(0))
        ));
    }
    v
}

fn to_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Format bytes as a short human string (e.g. `1.5 GiB`).
#[must_use]
#[allow(clippy::cast_precision_loss)] // display only
pub fn human(bytes: u64) -> String {
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else {
        format!("{} MB", bytes / MIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(ram_mb: u64, needs_vault: bool, vault_gb: u64) -> AppManifest {
        let mut m = AppManifest::from_yaml(
            "id: a\nname: a\ndescription: d\ncategory: c\nport: 4000\n\
             upstream: { service: w, port: 80 }\ncompose: |\n  services:\n    w:\n      image: x\n",
        )
        .expect("valid");
        m.resources.ram_mb = ram_mb;
        m.resources.needs_vault = needs_vault;
        m.resources.vault_gb = vault_gb;
        m
    }

    fn host(avail_gib: u64) -> HostSnapshot {
        HostSnapshot {
            mem_total: 8 * GIB,
            mem_available: avail_gib * GIB,
            disk_free: 40 * GIB,
            vault_free: Some(800 * GIB),
        }
    }

    #[test]
    fn small_app_on_idle_host_is_ok() {
        let v = evaluate(&app(200, false, 0), &host(5), &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Ok, "{:?}", v.reasons);
    }

    #[test]
    fn warns_when_headroom_gets_thin() {
        // 3 GiB free - 0.7 GiB = 2.3 GiB < 1.5 + 1.0 reserve+headroom
        let v = evaluate(&app(700, false, 0), &host(3), &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Warn, "{:?}", v.reasons);
    }

    #[test]
    fn denies_when_reserve_would_be_breached() {
        let v = evaluate(&app(1500, false, 0), &host(2), &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Deny);
    }

    #[test]
    fn denies_app_bigger_than_machine() {
        let v = evaluate(&app(16_000, false, 0), &host(7), &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Deny);
        assert!(v.reasons[0].contains("never spare"));
    }

    #[test]
    fn denies_vault_app_without_vault() {
        let mut h = host(5);
        h.vault_free = None;
        let v = evaluate(&app(100, true, 0), &h, &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Deny);
    }

    #[test]
    fn denies_on_low_internal_disk_even_if_ram_ok() {
        let mut h = host(5);
        h.disk_free = 5 * GIB;
        let v = evaluate(&app(100, false, 0), &h, &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Deny);
    }

    #[test]
    fn worst_reason_wins_and_all_reasons_kept() {
        let mut h = host(3);
        h.vault_free = Some(GIB);
        let v = evaluate(&app(700, true, 100), &h, &CapacityPolicy::default());
        assert_eq!(v.fit, Fit::Warn);
        assert_eq!(v.reasons.len(), 2);
    }

    #[test]
    fn human_formats() {
        assert_eq!(human(512 * MIB), "512 MB");
        assert_eq!(human(3 * GIB / 2), "1.5 GB");
    }
}
