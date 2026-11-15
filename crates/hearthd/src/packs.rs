//! Pack installs: one background job per pack, step by step, with live progress.
//!
//! Order: native Ollama (if the pack needs it) → models → apps. A failed step
//! doesn't stop the rest; the job reports which parts need attention.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use hearth_core::Catalog;
use hearth_core::pack::{MemberState, Pack, PackPlan, plan};
use hearth_host::ollama;
use serde::Serialize;

use crate::events::{Level, now};
use crate::state::SharedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub label: String,
    pub state: StepState,
    pub progress: Option<u8>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub pack: String,
    pub started: u64,
    pub finished: Option<u64>,
    pub steps: Vec<Step>,
}

impl Job {
    #[must_use]
    pub fn running(&self) -> bool {
        self.finished.is_none()
    }
}

#[derive(Debug, Default)]
pub struct Jobs(pub BTreeMap<String, Job>);

/// Load `catalog/packs/*.yml`, skipping (and logging) any that don't validate.
pub fn load(dir: &Path, catalog: &Catalog) -> Vec<Pack> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let pack = std::fs::read_to_string(&p).ok().and_then(|s| {
                Pack::from_yaml(&s)
                    .map_err(|e| tracing::warn!(path = %p.display(), "pack: {e}"))
                    .ok()
            })?;
            match pack.validate(catalog) {
                Ok(()) => Some(pack),
                Err(e) => {
                    tracing::warn!(path = %p.display(), "pack skipped: {e}");
                    None
                }
            }
        })
        .collect()
}

/// What's on the machine right now; gathered once per request.
pub struct Machine {
    pub apps: BTreeSet<String>,
    pub ollama: ollama::OllamaStatus,
}

impl Machine {
    pub async fn read(st: &SharedState, with_ollama: bool) -> Self {
        let (installed, _) = st.installed().await;
        let ollama = if with_ollama {
            ollama::status().await
        } else {
            ollama::OllamaStatus::default()
        };
        Self {
            apps: installed.into_iter().collect(),
            ollama,
        }
    }

    pub async fn plan(&self, st: &SharedState, pack: &Pack, with: &BTreeSet<String>) -> PackPlan {
        let models: BTreeSet<String> = self.ollama.models.iter().cloned().collect();
        plan(pack, &*st.catalog.read().await, with, &self.apps, &models)
    }
}

async fn set_step(st: &SharedState, pack: &str, i: usize, f: impl FnOnce(&mut Step)) {
    if let Some(s) = st
        .pack_jobs
        .write()
        .await
        .0
        .get_mut(pack)
        .and_then(|j| j.steps.get_mut(i))
    {
        f(s);
    }
}

enum Work {
    Ollama,
    Model(String),
    App(String),
}

/// Start installing; returns immediately. The job is visible via `GET /packs`.
pub async fn start(st: &SharedState, pack: Pack, with: BTreeSet<String>) {
    let m = Machine::read(st, pack.ollama.is_some()).await;
    let p = m.plan(st, &pack, &with).await;
    let ol = m.ollama;
    let mut work = Vec::new();
    let models: Vec<_> = p
        .models
        .iter()
        .filter(|m| m.state == MemberState::Install)
        .collect();
    if p.needs_ollama && (!ol.running || !models.is_empty()) {
        work.push((
            Work::Ollama,
            if ol.installed {
                "Start Ollama".to_owned()
            } else {
                "Install Ollama (native, uses the GPU)".to_owned()
            },
        ));
    }
    for m in models {
        work.push((
            Work::Model(m.name.clone()),
            format!("Download {} ({})", m.name, m.role),
        ));
    }
    for a in p.apps.iter().filter(|a| a.state == MemberState::Install) {
        work.push((Work::App(a.id.clone()), format!("Install {}", a.name)));
    }
    let steps = work
        .iter()
        .map(|(_, label)| Step {
            label: label.clone(),
            state: StepState::Pending,
            progress: None,
            error: None,
        })
        .collect();
    st.pack_jobs.write().await.0.insert(
        pack.id.clone(),
        Job {
            pack: pack.id.clone(),
            started: now(),
            finished: None,
            steps,
        },
    );

    tokio::spawn(run(st.clone(), pack, work));
}

/// Run the steps in order, recording progress on the job.
async fn run(st: SharedState, pack: Pack, work: Vec<(Work, String)>) {
    let id = pack.id.clone();
    let mut failed = 0;
    for (i, (w, _)) in work.into_iter().enumerate() {
        set_step(&st, &id, i, |s| s.state = StepState::Running).await;
        let result: anyhow::Result<()> = match w {
            Work::Ollama => ollama::install_and_start().await.map_err(Into::into),
            Work::Model(name) => {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
                let st2 = st.clone();
                let id2 = id.clone();
                let reporter = tokio::spawn(async move {
                    while let Some(pct) = rx.recv().await {
                        set_step(&st2, &id2, i, |s| s.progress = Some(pct)).await;
                    }
                });
                let r = ollama::pull(&name, move |pct| {
                    let _ = tx.send(pct);
                })
                .await;
                let _ = reporter.await;
                r.map_err(Into::into)
            }
            Work::App(app_id) => install_app(&st, &app_id).await,
        };
        match result {
            Ok(()) => set_step(&st, &id, i, |s| s.state = StepState::Done).await,
            Err(e) => {
                failed += 1;
                set_step(&st, &id, i, |s| {
                    s.state = StepState::Failed;
                    s.error = Some(format!("{e:#}"));
                })
                .await;
            }
        }
    }
    if let Some(j) = st.pack_jobs.write().await.0.get_mut(&id) {
        j.finished = Some(now());
    }
    if failed == 0 {
        st.event(
            Level::Success,
            "packs",
            format!("{} is ready", pack.name),
            pack.next_steps.first().cloned().unwrap_or_default(),
        )
        .await;
    } else {
        st.event(
            Level::Error,
            "packs",
            format!("{}: {failed} step(s) failed", pack.name),
            "Open the pack to see what went wrong and retry.",
        )
        .await;
    }
}

async fn install_app(st: &SharedState, id: &str) -> anyhow::Result<()> {
    let (app, config) = crate::api::apps::prepared(st, id)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let _ops = st.ops.lock().await;
    let (_, gateway) = crate::api::apps::deploy(st, &app, config.as_ref()).await?;
    gateway
        .map(|_| ())
        .map_err(|e| e.context("installed, but publishing it behind your login failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_pack_is_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
        let catalog = Catalog::load_dir(&root).expect("catalog");
        let files = std::fs::read_dir(root.join("packs"))
            .expect("packs dir")
            .count();
        let packs = load(&root.join("packs"), &catalog);
        assert_eq!(packs.len(), files, "a pack failed to load; see warnings");
        assert!(packs.iter().any(|p| p.id == "ai" && p.ollama.is_some()));
    }
}
