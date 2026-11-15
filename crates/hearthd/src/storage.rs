//! External drives: detection loop, attach/detach/eject, role links, auto pause/resume.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use hearth_core::storage::{
    self, Assessment, Attachment, Event as DriveEvent, Level as Suit, MARKER_VERSION, Marker,
    MatchedBy, Role, Snapshot, Status, Volume,
};
use hearth_host::volumes::{self, OFFLINE_TARGET};
use serde::Serialize;

use crate::events::{Level, now};
use crate::state::{AppState, SharedState};
use crate::store;

#[derive(Debug, Default)]
pub struct StorageState {
    pub attachments: Vec<Attachment>,
    pub volumes: Vec<Volume>,
    pub snapshot: Snapshot,
    pub scanned: bool,
    pub error: Option<String>,
}

impl StorageState {
    pub fn status(&self, att: &Attachment) -> Status {
        self.snapshot
            .statuses
            .get(&att.id)
            .cloned()
            .unwrap_or(Status::Offline { impostor_at: None })
    }

    /// Free bytes on the drive attached as `role`, if it's online.
    pub fn free_for(&self, role: Role) -> Option<u64> {
        let att = self.attachments.iter().find(|a| a.role == role)?;
        let mount = self.status(att).mount()?.clone();
        self.volumes
            .iter()
            .find(|v| v.mount == mount)
            .map(|v| v.free)
    }
}

fn path(st: &AppState) -> PathBuf {
    st.config.state_dir.join("storage.json")
}

pub fn load_attachments(state_dir: &Path) -> anyhow::Result<Vec<Attachment>> {
    store::load(&state_dir.join("storage.json"))
}

/// Rescan drives, update links and statuses, emit events, pause/resume dependent apps.
pub async fn refresh(st: &SharedState) -> anyhow::Result<()> {
    let vols = volumes::list().await.context("listing drives")?;
    let (events, changed_roles) = {
        let mut s = st.storage.write().await;
        let next = storage::snapshot(&s.attachments, &vols);
        let events = if s.scanned {
            storage::diff(&s.snapshot, &next, &s.attachments, &vols)
        } else {
            Vec::new()
        };

        // Remember where attached drives are; repair markers matched only by UUID.
        let mut dirty = false;
        let mut repairs = Vec::new();
        for att in &mut s.attachments {
            if let Some(Status::Online { mount, matched_by }) = next.statuses.get(&att.id) {
                if att.last_mount.as_ref() != Some(mount) {
                    att.last_mount = Some(mount.clone());
                    dirty = true;
                }
                att.last_seen = Some(now());
                if *matched_by == MatchedBy::VolumeUuid {
                    repairs.push((mount.clone(), marker_for(att)));
                }
            }
        }
        for (mount, marker) in repairs {
            if let Err(e) = volumes::write_marker(&mount, &marker).await {
                tracing::warn!(error = %e, "could not repair drive marker");
            }
        }
        // Point every role link at its drive, or at the offline sentinel.
        for role in Role::ALL {
            let target = s
                .attachments
                .iter()
                .find(|a| a.role == role)
                .and_then(|a| next.statuses.get(&a.id))
                .and_then(Status::mount)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(OFFLINE_TARGET));
            if let Err(e) = volumes::point_link(&st.config.role_link(role), &target) {
                tracing::error!(role = role.as_str(), error = %e, "could not update role link");
            }
        }
        let changed: Vec<(Role, bool)> = events
            .iter()
            .filter_map(|e| match e {
                DriveEvent::Reconnected { role, .. } => Some((*role, true)),
                DriveEvent::Disconnected { role, .. } => Some((*role, false)),
                _ => None,
            })
            .collect();
        s.volumes = vols;
        s.snapshot = next;
        s.scanned = true;
        s.error = None;
        if dirty {
            store::save(&path(st), &s.attachments)?;
        }
        (events, changed)
    };

    for e in &events {
        let level = match e.level() {
            Suit::Ok => Level::Info,
            Suit::Warn => Level::Warning,
            Suit::Deny => Level::Error,
        };
        let title = match e {
            DriveEvent::Reconnected { .. } => "Drive reconnected",
            DriveEvent::Disconnected { .. } => "Drive disconnected",
            DriveEvent::NewDrive { .. } => "New drive detected",
            DriveEvent::Impostor { .. } => "Different drive detected",
        };
        st.event(level, "storage", title, e.message()).await;
    }
    if st.settings.read().await.auto_pause_on_disconnect {
        for (role, online) in changed_roles {
            set_role_apps_running(st, role, online).await;
        }
    }
    Ok(())
}

/// Start or stop installed apps that depend on `role`.
async fn set_role_apps_running(st: &SharedState, role: Role, run: bool) {
    let (installed, _) = st.installed().await;
    let env = st.compose_env().await;
    let apps: Vec<_> = {
        let c = st.catalog.read().await;
        installed
            .iter()
            .filter_map(|id| c.get(id))
            .filter(|a| a.roles().contains(&role))
            .cloned()
            .collect()
    };
    for app in apps {
        let r = if run {
            st.compose.start(&app, &env).await
        } else {
            st.compose.stop(&app, &env).await
        };
        let verb = if run { "resumed" } else { "paused" };
        match r {
            Ok(()) => {
                st.event(
                    Level::Info,
                    "apps",
                    format!("{} {verb}", app.name),
                    format!("Because the {} drive changed.", role.as_str()),
                )
                .await;
            }
            Err(e) => {
                st.event(
                    Level::Error,
                    "apps",
                    format!(
                        "Couldn't {} {}",
                        if run { "resume" } else { "pause" },
                        app.name
                    ),
                    e.to_string(),
                )
                .await;
            }
        }
    }
}

fn marker_for(att: &Attachment) -> Marker {
    Marker {
        version: MARKER_VERSION,
        id: att.id.clone(),
        role: att.role,
        label: att.label.clone(),
        host: hostname(),
        attached_at: att.attached_at,
    }
}

fn hostname() -> String {
    sysinfo::System::host_name().unwrap_or_else(|_| "hearth".into())
}

/// Watch for mounts changing; full rescan on change or every 30 s (free space).
pub async fn run_watcher(st: SharedState) {
    let mut last = String::new();
    // `None` = never scanned: forces a full scan on the first tick.
    let mut last_full: Option<Instant> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(3));
    loop {
        tick.tick().await;
        let fp = volumes::mount_fingerprint().await;
        if fp != last || last_full.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            last = fp;
            last_full = Some(Instant::now());
            if let Err(e) = refresh(&st).await {
                tracing::warn!(error = %e, "storage refresh failed");
                st.storage.write().await.error = Some(format!("{e:#}"));
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AttachOutcome {
    pub attachment: Attachment,
    pub restored: bool,
    pub assessment: Assessment,
}

/// Attach the drive mounted at `mount` to `role`.
pub async fn attach(
    st: &SharedState,
    mount: &Path,
    role: Role,
    label: Option<String>,
    force: bool,
) -> anyhow::Result<AttachOutcome> {
    refresh(st).await?;
    let (vol, assessment) = {
        let s = st.storage.read().await;
        if let Some(existing) = s.attachments.iter().find(|a| a.role == role) {
            bail!(
                "“{}” is already the {} drive. Detach it first.",
                existing.label,
                role.as_str()
            );
        }
        let vol = s
            .volumes
            .iter()
            .find(|v| v.mount == mount)
            .cloned()
            .with_context(|| format!("No drive is mounted at {}.", mount.display()))?;
        if s.attachments
            .iter()
            .any(|a| matches!(s.status(a), Status::Online { mount: m, .. } if m == vol.mount))
        {
            bail!("This drive is already attached to another role.");
        }
        let a = storage::assess(&vol, role);
        (vol, a)
    };
    if assessment.level == Suit::Deny && !force {
        bail!("Not suitable: {}", assessment.notes.join(" "));
    }
    // Same drive coming back (marker present): keep its identity.
    let restored = vol.marker.is_some();
    let id = match &vol.marker {
        Some(m) => m.id.clone(),
        None => hearth_host::random_hex(12)?,
    };
    let att = Attachment {
        id,
        role,
        label: label
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| vol.name.clone()),
        volume_uuid: vol.volume_uuid.clone(),
        attached_at: now(),
        last_mount: Some(vol.mount.clone()),
        last_seen: Some(now()),
    };
    volumes::write_marker(&vol.mount, &marker_for(&att))
        .await
        .context("writing drive marker")?;
    {
        let mut s = st.storage.write().await;
        s.attachments.push(att.clone());
        store::save(&path(st), &s.attachments)?;
        s.scanned = false; // don't announce our own attach as a reconnect
    }
    refresh(st).await?;
    st.event(
        Level::Success,
        "storage",
        if restored {
            "Drive restored"
        } else {
            "Drive attached"
        },
        format!("{} is now the {} drive.", att.label, role.as_str()),
    )
    .await;
    Ok(AttachOutcome {
        attachment: att,
        restored,
        assessment,
    })
}

/// Installed apps that use `role`.
pub async fn dependents(st: &SharedState, role: Role) -> Vec<String> {
    let (installed, _) = st.installed().await;
    let c = st.catalog.read().await;
    installed
        .iter()
        .filter_map(|id| c.get(id))
        .filter(|a| a.roles().contains(&role))
        .map(|a| a.name.clone())
        .collect()
}

/// Forget an attachment. Refuses while installed apps depend on it.
pub async fn detach(st: &SharedState, id: &str) -> anyhow::Result<()> {
    let role = {
        let s = st.storage.read().await;
        s.attachments
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.role)
            .context("Unknown drive.")?
    };
    let users = dependents(st, role).await;
    if !users.is_empty() {
        bail!("Remove or stop these apps first: {}.", users.join(", "));
    }
    let label = {
        let mut s = st.storage.write().await;
        let label = s
            .attachments
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.label.clone())
            .unwrap_or_default();
        s.attachments.retain(|a| a.id != id);
        store::save(&path(st), &s.attachments)?;
        label
    };
    volumes::point_link(&st.config.role_link(role), Path::new(OFFLINE_TARGET))?;
    st.event(
        Level::Info,
        "storage",
        "Drive detached",
        format!(
            "{label} is no longer the {} drive. Its data is untouched.",
            role.as_str()
        ),
    )
    .await;
    refresh(st).await
}

/// Pause dependent apps, then safely eject.
pub async fn eject(st: &SharedState, id: &str) -> anyhow::Result<()> {
    let (role, vol) = {
        let s = st.storage.read().await;
        let att = s
            .attachments
            .iter()
            .find(|a| a.id == id)
            .context("Unknown drive.")?;
        let mount = s
            .status(att)
            .mount()
            .cloned()
            .context("The drive isn't connected.")?;
        let vol = s
            .volumes
            .iter()
            .find(|v| v.mount == mount)
            .cloned()
            .context("The drive isn't connected.")?;
        (att.role, vol)
    };
    set_role_apps_running(st, role, false).await;
    volumes::eject(&vol).await.context("ejecting")?;
    st.event(
        Level::Success,
        "storage",
        "Safe to unplug",
        format!("{} was ejected.", vol.name),
    )
    .await;
    refresh(st).await
}
