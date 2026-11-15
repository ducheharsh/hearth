//! Backups with restic (run in a container: nothing to install on the host).
//!
//! * Repository: the attached Backups drive, or S3-compatible / Backblaze B2.
//! * Per app: stop (optional) → `restic backup --tag app:<id>` → start → apply retention.
//! * Restore: stop → `restic restore <snap> --target / --delete` → start.
//! * Paths are mounted into the restic container at the *same* absolute path, so
//!   snapshots record real host paths and restore puts files exactly back.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, bail};
use hearth_core::AppManifest;
use hearth_core::backup::{Retention, paths_for};
use hearth_core::storage::Role;
use hearth_host::compose::path_str;
use hearth_host::run;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::{Level, now};
use crate::state::SharedState;
use crate::store;

pub const IMAGE: &str = "restic/restic:0.19.1";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Destination {
    /// The drive attached as Backups.
    #[default]
    Local,
    /// Any S3-compatible store (AWS, `MinIO`, R2, Wasabi…). `endpoint` like `s3.us-east-1.amazonaws.com`.
    S3 {
        endpoint: String,
        bucket: String,
        path: String,
    },
    /// Backblaze B2.
    B2 { bucket: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BackupSettings {
    pub enabled: bool,
    pub destination: Destination,
    /// Local hour (0-23) for the nightly run.
    pub hour: u8,
    pub retention: Retention,
    /// Stop each app while it's backed up (consistent databases).
    pub stop_apps: bool,
    pub include_media: bool,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            destination: Destination::Local,
            hour: 3,
            retention: Retention::default(),
            stop_apps: true,
            include_media: false,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Secrets {
    pub repo_password: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub at: u64,
    pub ok: bool,
    pub snapshot: Option<String>,
    pub bytes_added: Option<u64>,
    pub files: Option<u64>,
    pub error: Option<String>,
    pub seconds: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BackupState {
    pub settings: BackupSettings,
    pub last_nightly: Option<String>,
    pub results: BTreeMap<String, RunResult>,
    #[serde(skip)]
    pub running: BTreeSet<String>,
}

fn state_path(st: &SharedState) -> PathBuf {
    st.config.state_dir.join("backups.json")
}
fn secrets_path(st: &SharedState) -> PathBuf {
    st.config.secrets_dir().join("backup.json")
}

pub fn load(state_dir: &std::path::Path) -> BackupState {
    store::load(&state_dir.join("backups.json")).unwrap_or_default()
}

pub fn load_secrets(st: &SharedState) -> Secrets {
    store::load(&secrets_path(st)).unwrap_or_default()
}

pub fn save_secrets(st: &SharedState, s: &Secrets) -> anyhow::Result<()> {
    store::save(&secrets_path(st), s)
}

pub async fn save_state(st: &SharedState) -> anyhow::Result<()> {
    let b = st.backups.read().await;
    store::save(&state_path(st), &*b)
}

/// `docker run` arguments shared by every restic call (env + repository mount).
async fn base_args(st: &SharedState) -> anyhow::Result<(Vec<String>, Vec<(String, String)>)> {
    let settings = st.backups.read().await.settings.clone();
    let secrets = load_secrets(st);
    let password = secrets
        .repo_password
        .clone()
        .context("Set up backups first (no repository password yet).")?;
    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "--hostname".into(),
        "hearth".into(),
    ];
    let mut env = vec![("RESTIC_PASSWORD".to_owned(), password)];
    match &settings.destination {
        Destination::Local => {
            if st.storage.read().await.free_for(Role::Backups).is_none() {
                bail!("Attach a Backups drive in Storage (or choose S3/B2) before backing up.");
            }
            let link = st.config.role_link(Role::Backups);
            let real = std::fs::canonicalize(&link)
                .context("resolving the Backups drive")?
                .join("hearth-restic");
            std::fs::create_dir_all(&real)?;
            args.extend(["-v".into(), format!("{}:/repo", path_str(&real))]);
            env.push(("RESTIC_REPOSITORY".into(), "/repo".into()));
        }
        Destination::S3 {
            endpoint,
            bucket,
            path,
        } => {
            env.push((
                "RESTIC_REPOSITORY".into(),
                format!("s3:{endpoint}/{bucket}/{}", path.trim_matches('/')),
            ));
            env.push((
                "AWS_ACCESS_KEY_ID".into(),
                secrets
                    .access_key_id
                    .clone()
                    .context("Missing S3 access key")?,
            ));
            env.push((
                "AWS_SECRET_ACCESS_KEY".into(),
                secrets
                    .secret_access_key
                    .clone()
                    .context("Missing S3 secret key")?,
            ));
        }
        Destination::B2 { bucket, path } => {
            env.push((
                "RESTIC_REPOSITORY".into(),
                format!("b2:{bucket}:{}", path.trim_matches('/')),
            ));
            env.push((
                "B2_ACCOUNT_ID".into(),
                secrets.access_key_id.clone().context("Missing B2 key ID")?,
            ));
            env.push((
                "B2_ACCOUNT_KEY".into(),
                secrets
                    .secret_access_key
                    .clone()
                    .context("Missing B2 application key")?,
            ));
        }
    }
    for (k, _) in &env {
        args.extend(["-e".into(), k.clone()]);
    }
    Ok((args, env))
}

/// Run restic with extra bind mounts (`(path, read_only)`), returning stdout.
async fn restic(
    st: &SharedState,
    mounts: &[(PathBuf, bool)],
    restic_args: &[&str],
) -> anyhow::Result<String> {
    let (mut args, env) = base_args(st).await?;
    for (p, ro) in mounts {
        let s = path_str(p);
        args.extend([
            "-v".into(),
            format!("{s}:{s}{}", if *ro { ":ro" } else { "" }),
        ]);
    }
    args.push(IMAGE.into());
    args.extend(restic_args.iter().map(|s| (*s).to_owned()));
    let cmd: Vec<&str> = args.iter().map(String::as_str).collect();
    let envv: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    Ok(run("docker", &cmd, &envv).await?)
}

/// Create the repository (idempotent). Generates and stores the password if needed;
/// returns it so the UI can show it once.
pub async fn init(st: &SharedState) -> anyhow::Result<String> {
    let mut secrets = load_secrets(st);
    let password = if let Some(p) = &secrets.repo_password {
        p.clone()
    } else {
        let p = hearth_host::random_hex(24)?;
        secrets.repo_password = Some(p.clone());
        save_secrets(st, &secrets)?;
        p
    };
    if restic(st, &[], &["cat", "config"]).await.is_err() {
        restic(st, &[], &["init"])
            .await
            .context("creating the backup repository")?;
    }
    st.event(
        Level::Success,
        "backups",
        "Backup repository ready",
        "Save the recovery password somewhere safe.",
    )
    .await;
    Ok(password)
}

fn app_vars(st: &SharedState, app: &AppManifest) -> BTreeMap<String, String> {
    let mut v: BTreeMap<String, String> = Role::ALL
        .iter()
        .map(|r| (r.env_var().to_owned(), path_str(&st.config.role_link(*r))))
        .collect();
    v.insert(
        "APP_DATA".into(),
        path_str(&st.compose.app_dir(&app.id).join("data")),
    );
    v
}

/// Back up one app. `tag` adds an extra tag (e.g. `pre-update`).
pub async fn backup_app(
    st: &SharedState,
    app: &AppManifest,
    extra_tag: Option<&str>,
) -> anyhow::Result<RunResult> {
    let started = now();
    let include_media = st.backups.read().await.settings.include_media;
    let stop = st.backups.read().await.settings.stop_apps;
    let compose = std::fs::read_to_string(st.compose.app_dir(&app.id).join("compose.yml"))
        .unwrap_or_else(|_| app.compose.clone());
    let paths: Vec<PathBuf> = paths_for(
        &st.compose.app_dir(&app.id),
        &compose,
        &app_vars(st, app),
        include_media,
    )
    .into_iter()
    .filter(|p| p.exists())
    .collect();
    if paths.is_empty() {
        bail!("Nothing to back up for {}.", app.name);
    }
    st.backups.write().await.running.insert(app.id.clone());
    let (installed, running) = st.installed().await;
    let was_running = running.contains(&app.id);
    let env = st.compose_env().await;
    if stop && was_running {
        st.compose
            .stop(app, &env)
            .await
            .context("stopping app for a consistent backup")?;
    }
    let tag = format!("app:{}", app.id);
    let mut args = vec!["backup", "--json", "--tag", "hearth", "--tag", tag.as_str()];
    if let Some(t) = extra_tag {
        args.extend(["--tag", t]);
    }
    let path_strs: Vec<String> = paths.iter().map(|p| path_str(p)).collect();
    args.extend(path_strs.iter().map(String::as_str));
    let mounts: Vec<(PathBuf, bool)> = paths.iter().map(|p| (p.clone(), true)).collect();
    let res = restic(st, &mounts, &args).await;
    if stop && was_running && installed.contains(&app.id) {
        let _ = st.compose.start(app, &env).await;
    }
    st.backups.write().await.running.remove(&app.id);

    let result = match res {
        Ok(out) => {
            let summary = out
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .find(|v| v["message_type"] == "summary")
                .unwrap_or(Value::Null);
            RunResult {
                at: started,
                ok: true,
                snapshot: summary["snapshot_id"]
                    .as_str()
                    .map(|s| s.chars().take(8).collect()),
                bytes_added: summary["data_added"].as_u64(),
                files: summary["total_files_processed"].as_u64(),
                error: None,
                seconds: now() - started,
            }
        }
        Err(e) => RunResult {
            at: started,
            ok: false,
            snapshot: None,
            bytes_added: None,
            files: None,
            error: Some(format!("{e:#}")),
            seconds: now() - started,
        },
    };
    st.backups
        .write()
        .await
        .results
        .insert(app.id.clone(), result.clone());
    save_state(st).await?;
    if result.ok {
        st.event(
            Level::Success,
            "backups",
            format!("{} backed up", app.name),
            format!("Snapshot {}", result.snapshot.clone().unwrap_or_default()),
        )
        .await;
    } else {
        st.event(
            Level::Error,
            "backups",
            format!("Backup of {} failed", app.name),
            result.error.clone().unwrap_or_default(),
        )
        .await;
    }
    Ok(result)
}

/// Apply retention for one app's snapshots.
pub async fn prune_app(st: &SharedState, app_id: &str) -> anyhow::Result<()> {
    let r = st.backups.read().await.settings.retention;
    let tag = format!("app:{app_id}");
    let (d, w, m) = (
        r.daily.to_string(),
        r.weekly.to_string(),
        r.monthly.to_string(),
    );
    restic(
        st,
        &[],
        &[
            "forget",
            "--tag",
            &tag,
            "--group-by",
            "tags",
            "--keep-daily",
            &d,
            "--keep-weekly",
            &w,
            "--keep-monthly",
            &m,
            "--prune",
        ],
    )
    .await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub id: String,
    pub short_id: String,
    pub time: String,
    pub tags: Vec<String>,
    pub paths: Vec<String>,
    pub bytes: Option<u64>,
}

pub async fn snapshots(st: &SharedState, app_id: &str) -> anyhow::Result<Vec<Snapshot>> {
    let tag = format!("app:{app_id}");
    let out = restic(st, &[], &["snapshots", "--json", "--tag", &tag]).await?;
    let v: Vec<Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    let mut snaps: Vec<Snapshot> = v
        .iter()
        .map(|s| Snapshot {
            id: s["id"].as_str().unwrap_or_default().to_owned(),
            short_id: s["short_id"].as_str().unwrap_or_default().to_owned(),
            time: s["time"].as_str().unwrap_or_default().to_owned(),
            tags: s["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            paths: s["paths"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            bytes: s["summary"]["total_bytes_processed"].as_u64(),
        })
        .collect();
    snaps.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(snaps)
}

/// Put an app's files back exactly as they were in `snapshot` (removes newer files).
pub async fn restore(st: &SharedState, app: &AppManifest, snapshot: &str) -> anyhow::Result<()> {
    if snapshot.is_empty() || !snapshot.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Invalid snapshot id.");
    }
    let snaps = snapshots(st, &app.id).await?;
    let snap = snaps
        .iter()
        .find(|s| s.id.starts_with(snapshot) || s.short_id == snapshot)
        .context("That snapshot doesn't belong to this app.")?;
    let paths: Vec<PathBuf> = snap.paths.iter().map(PathBuf::from).collect();
    for p in &paths {
        std::fs::create_dir_all(p).ok();
    }
    let env = st.compose_env().await;
    let (installed, _) = st.installed().await;
    if installed.contains(&app.id) {
        st.compose
            .stop(app, &env)
            .await
            .context("stopping app before restore")?;
    }
    let mounts: Vec<(PathBuf, bool)> = paths.iter().map(|p| (p.clone(), false)).collect();
    // `--delete` at `/` must be scoped: only touch this snapshot's own paths.
    let mut args: Vec<&str> = vec!["restore", &snap.id, "--target", "/", "--delete"];
    for p in &snap.paths {
        args.extend(["--include", p.as_str()]);
    }
    let res = restic(st, &mounts, &args).await;
    if installed.contains(&app.id) {
        st.compose.start(app, &env).await.ok();
    }
    res.context("restoring")?;
    st.event(
        Level::Success,
        "backups",
        format!("{} restored", app.name),
        format!("From snapshot {} ({})", snap.short_id, snap.time),
    )
    .await;
    Ok(())
}

/// Back up every installed app, then apply retention.
pub async fn run_all(st: &SharedState) -> anyhow::Result<()> {
    let (installed, _) = st.installed().await;
    let apps: Vec<AppManifest> = {
        let c = st.catalog.read().await;
        installed
            .iter()
            .filter_map(|id| c.get(id))
            .cloned()
            .collect()
    };
    for app in &apps {
        let _ops = st.ops.lock().await;
        if backup_app(st, app, Some("nightly")).await.is_ok() {
            if let Err(e) = prune_app(st, &app.id).await {
                tracing::warn!(app = %app.id, error = %e, "retention failed");
            }
        }
    }
    Ok(())
}

/// Nightly scheduler: once per day at the configured local hour.
pub async fn run_scheduler(st: SharedState) {
    let mut tick = tokio::time::interval(Duration::from_secs(300));
    loop {
        tick.tick().await;
        let (enabled, hour, last) = {
            let b = st.backups.read().await;
            (b.settings.enabled, b.settings.hour, b.last_nightly.clone())
        };
        if !enabled {
            continue;
        }
        let Ok(stamp) = run("date", &["+%Y-%m-%d %H"], &[]).await else {
            continue;
        };
        let mut it = stamp.split_whitespace();
        let (Some(day), Some(h)) = (it.next(), it.next().and_then(|h| h.parse::<u8>().ok())) else {
            continue;
        };
        if h != hour || last.as_deref() == Some(day) {
            continue;
        }
        st.backups.write().await.last_nightly = Some(day.to_owned());
        let _ = save_state(&st).await;
        if let Err(e) = run_all(&st).await {
            st.event(
                Level::Error,
                "backups",
                "Nightly backup failed",
                format!("{e:#}"),
            )
            .await;
        }
    }
}
