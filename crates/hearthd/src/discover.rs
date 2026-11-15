//! Keeps a shallow clone of awesome-selfhosted-data and serves it to Discover.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use hearth_core::discover::{Entry, parse_entry};
use hearth_host::run;

use crate::events::{Level, now};
use crate::state::SharedState;

pub const REPO: &str = "https://github.com/awesome-selfhosted/awesome-selfhosted-data.git";
const EVERY: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Default)]
pub struct DiscoverState {
    pub entries: Vec<Entry>,
    pub synced_at: Option<u64>,
    pub syncing: bool,
    pub error: Option<String>,
}

pub fn data_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("discover").join("awesome-selfhosted-data")
}

/// Clone or fast-forward the data repo (shallow).
async fn fetch(dir: &Path) -> anyhow::Result<()> {
    let d = dir.to_string_lossy().into_owned();
    if dir.join(".git").exists() {
        run(
            "git",
            &["-C", &d, "fetch", "--depth", "1", "origin", "HEAD"],
            &[],
        )
        .await?;
        run("git", &["-C", &d, "reset", "--hard", "FETCH_HEAD"], &[]).await?;
    } else {
        if let Some(parent) = dir.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        run("git", &["clone", "--depth", "1", REPO, &d], &[]).await?;
    }
    Ok(())
}

/// Parse every `software/*.yml` (blocking; call via `spawn_blocking`).
pub fn parse_dir(dir: &Path) -> anyhow::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir.join("software")).context("reading software/")? {
        let p = e?.path();
        let Some(slug) = p.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        if p.extension().is_none_or(|x| x != "yml") {
            continue;
        }
        let has_icon = dir.join("icons").join(format!("{slug}.webp")).exists();
        match std::fs::read_to_string(&p)
            .map_err(anyhow::Error::from)
            .and_then(|y| Ok(parse_entry(&slug, &y, has_icon)?))
        {
            Ok(entry) => out.push(entry),
            Err(err) => tracing::debug!(%slug, error = %err, "skipping directory entry"),
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Load whatever is on disk (fast startup), without network.
pub async fn load_local(st: &SharedState) {
    let dir = data_dir(&st.config.state_dir);
    if !dir.join("software").exists() {
        return;
    }
    if let Ok(Ok(entries)) = tokio::task::spawn_blocking(move || parse_dir(&dir)).await {
        let mut d = st.discover.write().await;
        d.entries = entries;
    }
}

pub async fn sync(st: &SharedState) -> anyhow::Result<usize> {
    st.discover.write().await.syncing = true;
    let dir = data_dir(&st.config.state_dir);
    let res = async {
        fetch(&dir).await?;
        let d2 = dir.clone();
        tokio::task::spawn_blocking(move || parse_dir(&d2)).await?
    }
    .await;
    let mut d = st.discover.write().await;
    d.syncing = false;
    match res {
        Ok(entries) => {
            let n = entries.len();
            d.entries = entries;
            d.synced_at = Some(now());
            d.error = None;
            Ok(n)
        }
        Err(e) => {
            d.error = Some(format!("{e:#}"));
            Err(e)
        }
    }
}

pub async fn run_syncer(st: SharedState) {
    load_local(&st).await;
    loop {
        if st.settings.read().await.discover_enabled {
            match sync(&st).await {
                Ok(n) => tracing::info!(apps = n, "discover directory synced"),
                Err(e) => {
                    tracing::warn!(error = %e, "discover sync failed");
                    st.event(
                        Level::Warning,
                        "discover",
                        "Directory sync failed",
                        format!("{e:#}"),
                    )
                    .await;
                }
            }
        }
        tokio::time::sleep(EVERY).await;
    }
}
