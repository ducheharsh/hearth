//! Per-app resource history: container stats aggregated per compose project into
//! 5-minute buckets, kept for 7 days, persisted to `state_dir/history.json`.
//!
//! Feeds the app detail charts and gives the capacity planner *measured* memory
//! (p95) instead of the manifest's estimate once an app has run for a while.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::events::now;
use crate::state::SharedState;
use crate::store;

pub const BUCKET_SECS: u64 = 300;
pub const KEEP: usize = 7 * 24 * 12; // 7 days of 5-minute buckets
const SAMPLE_EVERY: Duration = Duration::from_secs(30);
/// Need at least an hour of data before trusting measurements over the manifest.
const MIN_BUCKETS_FOR_ESTIMATE: usize = 12;

/// One finished bucket: start time, average memory (bytes), average CPU (%).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Bucket(pub u64, pub u64, pub f32);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    /// Compose project (e.g. `hearth-immich`) → buckets, oldest first.
    pub apps: BTreeMap<String, VecDeque<Bucket>>,
    #[serde(skip)]
    open: BTreeMap<String, Acc>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Acc {
    start: u64,
    mem_sum: u128,
    cpu_sum: f64,
    n: u32,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub points: Vec<Bucket>,
    pub avg_mem: u64,
    pub p95_mem: u64,
    pub peak_mem: u64,
    pub avg_cpu: f32,
    pub hours: f64,
}

impl History {
    /// Record one sample for `project` at time `t`.
    pub fn record(&mut self, project: &str, t: u64, mem: u64, cpu: f32) {
        let start = t - t % BUCKET_SECS;
        let acc = self.open.entry(project.to_owned()).or_insert(Acc {
            start,
            ..Acc::default()
        });
        if acc.start != start && acc.n > 0 {
            let done = *acc;
            let series = self.apps.entry(project.to_owned()).or_default();
            series.push_back(Bucket(
                done.start,
                u64::try_from(done.mem_sum / u128::from(done.n)).unwrap_or(u64::MAX),
                #[allow(clippy::cast_possible_truncation)]
                {
                    (done.cpu_sum / f64::from(done.n)) as f32
                },
            ));
            while series.len() > KEEP {
                series.pop_front();
            }
            *acc = Acc {
                start,
                ..Acc::default()
            };
        }
        acc.start = start;
        acc.mem_sum += u128::from(mem);
        acc.cpu_sum += f64::from(cpu);
        acc.n += 1;
    }

    #[must_use]
    pub fn summary(&self, project: &str) -> Option<Summary> {
        let s = self.apps.get(project)?;
        if s.is_empty() {
            return None;
        }
        let mut mems: Vec<u64> = s.iter().map(|b| b.1).collect();
        mems.sort_unstable();
        let p95 = mems[((mems.len() - 1) * 95) / 100];
        let n = s.len() as u64;
        #[allow(clippy::cast_precision_loss)]
        Some(Summary {
            avg_mem: mems.iter().sum::<u64>() / n,
            p95_mem: p95,
            peak_mem: *mems.last().unwrap_or(&0),
            avg_cpu: s.iter().map(|b| b.2).sum::<f32>() / n as f32,
            hours: (n * BUCKET_SECS) as f64 / 3600.0,
            points: s.iter().copied().collect(),
        })
    }

    /// Measured RAM (MB) for capacity planning, once there's enough data.
    #[must_use]
    pub fn measured_ram_mb(&self, project: &str) -> Option<u64> {
        let s = self.apps.get(project)?;
        (s.len() >= MIN_BUCKETS_FOR_ESTIMATE)
            .then(|| self.summary(project).map(|x| x.p95_mem / (1024 * 1024)))?
    }
}

fn path(st: &SharedState) -> std::path::PathBuf {
    st.config.state_dir.join("history.json")
}

pub fn load(state_dir: &std::path::Path) -> History {
    store::load(&state_dir.join("history.json")).unwrap_or_default()
}

/// Sample container usage forever; persist every few minutes.
pub async fn run_recorder(st: SharedState) {
    let mut tick = tokio::time::interval(SAMPLE_EVERY);
    let mut last_save = now();
    loop {
        tick.tick().await;
        let Some(docker) = &st.docker else { continue };
        let Ok(list) = docker.containers().await else {
            continue;
        };
        let mut per: BTreeMap<String, (u64, f32)> = BTreeMap::new();
        for c in list.iter().filter(|c| c.state == "running") {
            let key = c.project.clone().unwrap_or_else(|| c.name.clone());
            let e = per.entry(key).or_default();
            e.0 += c.mem_bytes.unwrap_or(0);
            #[allow(clippy::cast_possible_truncation)]
            {
                e.1 += c.cpu_percent.unwrap_or(0.0) as f32;
            }
        }
        let t = now();
        {
            let mut h = st.history.write().await;
            for (project, (mem, cpu)) in per {
                h.record(&project, t, mem, cpu);
            }
            if t - last_save >= BUCKET_SECS {
                if let Err(e) = store::save(&path(&st), &*h) {
                    tracing::warn!(error = %e, "saving history failed");
                }
                last_save = t;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_average_and_roll_over() {
        let mut h = History::default();
        h.record("p", 0, 100, 10.0);
        h.record("p", 60, 300, 30.0);
        assert!(h.summary("p").is_none(), "bucket still open");
        h.record("p", 300, 50, 5.0); // next bucket closes the first
        let s = h.summary("p").expect("one bucket");
        assert_eq!(s.points, vec![Bucket(0, 200, 20.0)]);
        assert_eq!(s.p95_mem, 200);
    }

    #[test]
    fn keeps_seven_days_and_needs_an_hour_for_estimates() {
        let mut h = History::default();
        for i in 0..(KEEP as u64 + 20) {
            h.record("p", i * BUCKET_SECS, 1024 * 1024 * (i % 10 + 1), 1.0);
        }
        assert_eq!(h.apps["p"].len(), KEEP);
        assert!(h.measured_ram_mb("p").is_some_and(|m| m >= 9));
        let mut short = History::default();
        for i in 0..5 {
            short.record("q", i * BUCKET_SECS, 1, 1.0);
        }
        assert!(short.measured_ram_mb("q").is_none());
    }
}
