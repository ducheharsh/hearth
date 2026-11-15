//! Host metrics. Keep one [`SystemSampler`] alive and call [`SystemSampler::sample`]
//! periodically: CPU usage is computed from the delta between refreshes.

use std::path::{Path, PathBuf};

use hearth_core::HostSnapshot;
use serde::Serialize;
use sysinfo::{Disks, System};

/// One reading of the host.
#[derive(Debug, Clone, Serialize)]
pub struct SystemSample {
    /// Unix seconds.
    pub at: u64,
    pub hostname: String,
    pub os: String,
    pub uptime_secs: u64,
    pub cpu_count: usize,
    /// 0-100, averaged across cores.
    pub cpu_percent: f32,
    pub load_avg: [f64; 3],
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disks: Vec<DiskUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskUsage {
    pub mount: PathBuf,
    pub total: u64,
    pub free: u64,
}

impl SystemSample {
    /// Reduce to what the capacity planner needs. `vault_free` comes from the storage manager.
    #[must_use]
    pub fn snapshot(&self, vault_free: Option<u64>) -> HostSnapshot {
        HostSnapshot {
            mem_total: self.mem_total,
            mem_available: self.mem_available,
            disk_free: self
                .disks
                .iter()
                .find(|d| d.mount == Path::new("/"))
                .map_or(0, |d| d.free),
            vault_free,
        }
    }
}

/// Long-lived sampler (holds `sysinfo` state between refreshes).
#[derive(Debug)]
pub struct SystemSampler {
    sys: System,
    disks: Disks,
}

impl SystemSampler {
    /// Fails only if the platform is unsupported by `sysinfo`.
    pub fn new() -> Result<Self, sysinfo::Error> {
        let mut sys = System::new()?;
        sys.refresh_cpu_usage(); // prime the CPU delta
        Ok(Self {
            sys,
            disks: Disks::new_with_refreshed_list()?,
        })
    }

    /// Refresh and read. Blocking (fast, but call via `spawn_blocking` from async code).
    pub fn sample(&mut self) -> SystemSample {
        self.sys.refresh_memory();
        self.sys.refresh_cpu_usage();
        self.disks.refresh(true);
        let load = System::load_average().map_or([0.0; 3], |l| [l.one, l.five, l.fifteen]);

        let disks = self
            .disks
            .iter()
            .filter(|d| is_interesting_mount(d.mount_point()))
            .map(|d| DiskUsage {
                mount: d.mount_point().to_path_buf(),
                total: d.total_space(),
                free: d.available_space(),
            })
            .collect();

        SystemSample {
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            hostname: System::host_name().unwrap_or_default(),
            os: System::long_os_version().unwrap_or_default(),
            uptime_secs: System::uptime().unwrap_or(0),
            cpu_count: self.sys.cpus().len(),
            cpu_percent: self.sys.global_cpu_usage(),
            load_avg: load,
            mem_total: self.sys.total_memory(),
            mem_used: self.sys.used_memory(),
            mem_available: self.sys.available_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            disks,
        }
    }
}

/// Skip macOS system sub-volumes; keep the root and user-visible volumes.
fn is_interesting_mount(p: &Path) -> bool {
    p == Path::new("/") || (p.starts_with("/Volumes") && !p.starts_with("/Volumes/Recovery"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_real_host() {
        let mut s = SystemSampler::new().expect("supported platform");
        let sample = s.sample();
        assert!(sample.mem_total > 0);
        assert!(sample.cpu_count > 0);
        assert!(sample.disks.iter().any(|d| d.mount == Path::new("/")));
    }

    #[test]
    fn snapshot_reports_missing_vault() {
        let mut s = SystemSampler::new().expect("supported platform");
        let snap = s.sample().snapshot(None);
        assert!(snap.vault_free.is_none());
        assert!(snap.disk_free > 0);
    }

    #[test]
    fn mount_filter() {
        assert!(is_interesting_mount(Path::new("/")));
        assert!(is_interesting_mount(Path::new("/Volumes/Vault")));
        assert!(!is_interesting_mount(Path::new("/System/Volumes/Data")));
    }
}
