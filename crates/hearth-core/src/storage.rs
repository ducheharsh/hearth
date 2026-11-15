//! External storage: identifying drives, deciding if a drive is "the same one",
//! checking suitability, and turning state changes into events.
//!
//! Identity rules, strongest first:
//! 1. **Marker**: hearth writes `.hearth-volume.json` (with a random id) to the drive root
//!    when it is attached. Survives renames and remounts at a different path.
//! 2. **Volume UUID**: from the OS. Survives renames; changes when the drive is reformatted.
//! 3. The mount path/name is never used for identity, so a different drive that happens
//!    to be called "Vault" is reported as an impostor, not used.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const GIB: u64 = 1024 * 1024 * 1024;

/// What a drive is used for. Each role is exposed to apps as a stable path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// App bulk data: photos, documents, sync folders (`${VAULT}`).
    Vault,
    /// Backup repositories (`${BACKUPS}`).
    Backups,
    /// Movies/music libraries (`${MEDIA}`).
    Media,
}

impl Role {
    pub const ALL: [Self; 3] = [Self::Vault, Self::Backups, Self::Media];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Backups => "backups",
            Self::Media => "media",
        }
    }

    /// Compose variable name for this role.
    #[must_use]
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::Vault => "VAULT",
            Self::Backups => "BACKUPS",
            Self::Media => "MEDIA",
        }
    }

    /// Recommended minimum size.
    const fn min_size(self) -> u64 {
        match self {
            Self::Vault | Self::Media => 64 * GIB,
            Self::Backups => 32 * GIB,
        }
    }
}

/// Contents of `.hearth-volume.json` on the drive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub version: u32,
    /// Random id assigned at attach time; the strongest identity signal.
    pub id: String,
    pub role: Role,
    pub label: String,
    /// Host that attached it (helps when moving drives between machines).
    pub host: String,
    pub attached_at: u64,
}

pub const MARKER_FILE: &str = ".hearth-volume.json";
pub const MARKER_VERSION: u32 = 1;

/// A mounted external volume as reported by the OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Volume {
    pub mount: PathBuf,
    pub name: String,
    pub volume_uuid: Option<String>,
    /// e.g. `apfs`, `exfat`, `msdos`, `hfs`, `ext4`.
    pub filesystem: String,
    /// e.g. `USB`, `Thunderbolt`, `Disk Image`.
    pub protocol: Option<String>,
    pub device: String,
    pub writable: bool,
    pub ejectable: bool,
    pub total: u64,
    pub free: u64,
    /// Parsed `.hearth-volume.json`, if the drive has one.
    pub marker: Option<Marker>,
}

/// A drive the user attached to a role. Persisted by hearth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Same as the marker id written to the drive.
    pub id: String,
    pub role: Role,
    pub label: String,
    pub volume_uuid: Option<String>,
    pub attached_at: u64,
    /// Where it was last seen mounted (informational).
    pub last_mount: Option<PathBuf>,
    pub last_seen: Option<u64>,
}

/// How an attachment matched a mounted volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedBy {
    Marker,
    /// UUID matched but the marker was missing/foreign; it should be rewritten.
    VolumeUuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Online {
        mount: PathBuf,
        matched_by: MatchedBy,
    },
    Offline {
        /// A *different* drive is mounted where this one used to be.
        impostor_at: Option<PathBuf>,
    },
}

impl Status {
    #[must_use]
    pub fn mount(&self) -> Option<&PathBuf> {
        match self {
            Self::Online { mount, .. } => Some(mount),
            Self::Offline { .. } => None,
        }
    }
}

/// Find the mounted volume for an attachment.
#[must_use]
pub fn resolve(att: &Attachment, volumes: &[Volume]) -> Status {
    if let Some(v) = volumes
        .iter()
        .find(|v| v.marker.as_ref().is_some_and(|m| m.id == att.id))
    {
        return Status::Online {
            mount: v.mount.clone(),
            matched_by: MatchedBy::Marker,
        };
    }
    if let Some(uuid) = &att.volume_uuid
        && let Some(v) = volumes
            .iter()
            .find(|v| v.volume_uuid.as_deref() == Some(uuid.as_str()))
    {
        return Status::Online {
            mount: v.mount.clone(),
            matched_by: MatchedBy::VolumeUuid,
        };
    }
    let impostor_at = att
        .last_mount
        .as_ref()
        .filter(|p| volumes.iter().any(|v| &v.mount == *p))
        .cloned();
    Status::Offline { impostor_at }
}

/// Severity of a suitability check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    pub level: Level,
    pub notes: Vec<String>,
}

/// Is `v` a good home for `role`?
#[must_use]
pub fn assess(v: &Volume, role: Role) -> Assessment {
    let mut notes = Vec::new();
    let mut level = Level::Ok;
    let mut raise = |l: Level, msg: String, notes: &mut Vec<String>| {
        level = level.max(l);
        notes.push(msg);
    };

    if !v.writable {
        raise(Level::Deny, "The drive is read-only.".into(), &mut notes);
    }
    match v.filesystem.to_ascii_lowercase().as_str() {
        "apfs" | "ext4" | "btrfs" | "xfs" | "zfs" => {}
        "hfs" => raise(
            Level::Warn,
            "Mac OS Extended works, but APFS is faster and safer on SSDs.".into(),
            &mut notes,
        ),
        "exfat" => raise(
            Level::Warn,
            "exFAT has no file permissions or journaling; databases can corrupt on unplug. Reformat as APFS for app data."
                .into(),
            &mut notes,
        ),
        "msdos" | "fat32" | "vfat" => raise(
            Level::Deny,
            "FAT32 can't store files over 4 GB and has no permissions. Reformat as APFS.".into(),
            &mut notes,
        ),
        other => raise(
            Level::Warn,
            format!("Unrecognised filesystem `{other}`."),
            &mut notes,
        ),
    }
    if v.total < role.min_size() {
        raise(
            Level::Warn,
            format!(
                "Only {} GB; at least {} GB is recommended for {}.",
                v.total / GIB,
                role.min_size() / GIB,
                role.as_str()
            ),
            &mut notes,
        );
    }
    if v.total > 0 && v.free * 10 < v.total {
        raise(
            Level::Warn,
            "Less than 10% free space left.".into(),
            &mut notes,
        );
    }
    if let Some(m) = &v.marker
        && m.role != role
    {
        raise(
            Level::Warn,
            format!(
                "This drive was previously the {} drive (\u{201c}{}\u{201d}).",
                m.role.as_str(),
                m.label
            ),
            &mut notes,
        );
    }
    if notes.is_empty() {
        notes.push("Looks good.".into());
    }
    Assessment { level, notes }
}

/// Everything the watcher knows at one moment. Compared to produce events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Mount paths of all detected volumes.
    pub mounts: BTreeSet<PathBuf>,
    /// Attachment id → status.
    pub statuses: BTreeMap<String, Status>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// An attached drive came back (possibly at a different path).
    Reconnected {
        role: Role,
        label: String,
        mount: PathBuf,
    },
    /// An attached drive disappeared.
    Disconnected { role: Role, label: String },
    /// A drive that isn't attached appeared.
    NewDrive {
        name: String,
        mount: PathBuf,
        known_role: Option<Role>,
    },
    /// A different drive sits where an attached one used to be.
    Impostor {
        role: Role,
        label: String,
        mount: PathBuf,
    },
}

impl Event {
    #[must_use]
    pub fn level(&self) -> Level {
        match self {
            Self::Reconnected { .. } | Self::NewDrive { .. } => Level::Ok,
            Self::Disconnected { .. } => Level::Warn,
            Self::Impostor { .. } => Level::Deny,
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Reconnected { role, label, mount } => format!(
                "{label} is back ({} drive) at {}.",
                role.as_str(),
                mount.display()
            ),
            Self::Disconnected { role, label } => format!(
                "{label} ({} drive) was disconnected. Apps that use it are paused.",
                role.as_str()
            ),
            Self::NewDrive {
                name,
                known_role: Some(r),
                ..
            } => {
                format!(
                    "{name} was connected. It was a {} drive before; attach it to restore.",
                    r.as_str()
                )
            }
            Self::NewDrive { name, .. } => format!("{name} was connected."),
            Self::Impostor { role, label, mount } => format!(
                "A different drive is mounted at {} where {label} ({} drive) used to be. hearth won't use it.",
                mount.display(),
                role.as_str()
            ),
        }
    }
}

/// Events implied by moving from `prev` to `next`.
#[must_use]
pub fn diff(
    prev: &Snapshot,
    next: &Snapshot,
    attachments: &[Attachment],
    volumes: &[Volume],
) -> Vec<Event> {
    let mut events = Vec::new();
    for att in attachments {
        let (before, after) = (prev.statuses.get(&att.id), next.statuses.get(&att.id));
        match (before, after) {
            (Some(Status::Offline { .. }) | None, Some(Status::Online { mount, .. }))
                if before.is_some() =>
            {
                events.push(Event::Reconnected {
                    role: att.role,
                    label: att.label.clone(),
                    mount: mount.clone(),
                });
            }
            (Some(Status::Online { .. }), Some(Status::Offline { .. })) => {
                events.push(Event::Disconnected {
                    role: att.role,
                    label: att.label.clone(),
                });
            }
            _ => {}
        }
        if let Some(Status::Offline {
            impostor_at: Some(at),
        }) = after
        {
            let was = matches!(before, Some(Status::Offline { impostor_at: Some(p) }) if p == at);
            if !was {
                events.push(Event::Impostor {
                    role: att.role,
                    label: att.label.clone(),
                    mount: at.clone(),
                });
            }
        }
    }

    let attached_mounts: BTreeSet<&PathBuf> =
        next.statuses.values().filter_map(Status::mount).collect();
    for v in volumes {
        if !prev.mounts.contains(&v.mount) && !attached_mounts.contains(&v.mount) {
            let is_impostor = next
                .statuses
                .values()
                .any(|s| matches!(s, Status::Offline { impostor_at: Some(p) } if p == &v.mount));
            if !is_impostor {
                events.push(Event::NewDrive {
                    name: v.name.clone(),
                    mount: v.mount.clone(),
                    known_role: v.marker.as_ref().map(|m| m.role),
                });
            }
        }
    }
    events
}

/// Build a snapshot from the current volumes and attachments.
#[must_use]
pub fn snapshot(attachments: &[Attachment], volumes: &[Volume]) -> Snapshot {
    Snapshot {
        mounts: volumes.iter().map(|v| v.mount.clone()).collect(),
        statuses: attachments
            .iter()
            .map(|a| (a.id.clone(), resolve(a, volumes)))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(name: &str, uuid: &str, marker_id: Option<&str>) -> Volume {
        Volume {
            mount: PathBuf::from(format!("/Volumes/{name}")),
            name: name.into(),
            volume_uuid: Some(uuid.into()),
            filesystem: "apfs".into(),
            protocol: Some("USB".into()),
            device: "disk4s1".into(),
            writable: true,
            ejectable: true,
            total: 1000 * GIB,
            free: 800 * GIB,
            marker: marker_id.map(|id| Marker {
                version: MARKER_VERSION,
                id: id.into(),
                role: Role::Vault,
                label: "Vault".into(),
                host: "m1".into(),
                attached_at: 0,
            }),
        }
    }

    fn att(id: &str, uuid: &str) -> Attachment {
        Attachment {
            id: id.into(),
            role: Role::Vault,
            label: "Vault".into(),
            volume_uuid: Some(uuid.into()),
            attached_at: 0,
            last_mount: Some("/Volumes/Vault".into()),
            last_seen: None,
        }
    }

    #[test]
    fn marker_wins_even_if_renamed() {
        let a = att("abc", "U1");
        let v = vol("Vault 1", "U1", Some("abc"));
        assert_eq!(
            resolve(&a, &[v]),
            Status::Online {
                mount: "/Volumes/Vault 1".into(),
                matched_by: MatchedBy::Marker
            }
        );
    }

    #[test]
    fn uuid_fallback_when_marker_missing() {
        let a = att("abc", "U1");
        let v = vol("Vault", "U1", None);
        assert!(matches!(
            resolve(&a, &[v]),
            Status::Online {
                matched_by: MatchedBy::VolumeUuid,
                ..
            }
        ));
    }

    #[test]
    fn same_name_different_drive_is_impostor() {
        let a = att("abc", "U1");
        let v = vol("Vault", "OTHER", None);
        assert_eq!(
            resolve(&a, &[v]),
            Status::Offline {
                impostor_at: Some("/Volumes/Vault".into())
            }
        );
    }

    #[test]
    fn reformatted_drive_is_not_the_same() {
        // Reformat: new UUID, marker gone.
        let a = att("abc", "U1");
        let v = vol("Elsewhere", "U2", None);
        assert_eq!(resolve(&a, &[v]), Status::Offline { impostor_at: None });
    }

    #[test]
    fn assess_filesystems() {
        let mut v = vol("X", "U", None);
        assert_eq!(assess(&v, Role::Vault).level, Level::Ok);
        v.filesystem = "exfat".into();
        assert_eq!(assess(&v, Role::Vault).level, Level::Warn);
        v.filesystem = "msdos".into();
        assert_eq!(assess(&v, Role::Vault).level, Level::Deny);
        v.filesystem = "apfs".into();
        v.writable = false;
        assert_eq!(assess(&v, Role::Vault).level, Level::Deny);
    }

    #[test]
    fn assess_warns_on_role_change_and_small_drive() {
        let mut v = vol("X", "U", Some("id"));
        v.total = 16 * GIB;
        v.free = 15 * GIB;
        let a = assess(&v, Role::Media);
        assert_eq!(a.level, Level::Warn);
        assert_eq!(a.notes.len(), 2, "{:?}", a.notes);
    }

    #[test]
    fn diff_reports_disconnect_then_reconnect_at_new_path() {
        let a = att("abc", "U1");
        let online = vec![vol("Vault", "U1", Some("abc"))];
        let s1 = snapshot(std::slice::from_ref(&a), &online);
        let s2 = snapshot(std::slice::from_ref(&a), &[]);
        let e = diff(&s1, &s2, std::slice::from_ref(&a), &[]);
        assert!(
            matches!(e.as_slice(), [Event::Disconnected { .. }]),
            "{e:?}"
        );

        let back = vec![vol("Vault 1", "U1", Some("abc"))];
        let s3 = snapshot(std::slice::from_ref(&a), &back);
        let e = diff(&s2, &s3, std::slice::from_ref(&a), &back);
        assert!(
            matches!(e.as_slice(), [Event::Reconnected { mount, .. }] if mount == &PathBuf::from("/Volumes/Vault 1")),
            "{e:?}"
        );
    }

    #[test]
    fn diff_flags_impostor_once_and_not_as_new_drive() {
        let a = att("abc", "U1");
        let s0 = snapshot(std::slice::from_ref(&a), &[]);
        let fake = vec![vol("Vault", "OTHER", None)];
        let s1 = snapshot(std::slice::from_ref(&a), &fake);
        let e = diff(&s0, &s1, std::slice::from_ref(&a), &fake);
        assert!(matches!(e.as_slice(), [Event::Impostor { .. }]), "{e:?}");
        assert_eq!(diff(&s1, &s1, std::slice::from_ref(&a), &fake), Vec::new());
    }

    #[test]
    fn diff_new_drive_mentions_previous_role() {
        let v = vec![vol("Old", "U9", Some("zzz"))];
        let e = diff(&Snapshot::default(), &snapshot(&[], &v), &[], &v);
        assert!(matches!(
            e.as_slice(),
            [Event::NewDrive {
                known_role: Some(Role::Vault),
                ..
            }]
        ));
    }

    #[test]
    fn first_snapshot_does_not_announce_reconnects() {
        let a = att("abc", "U1");
        let v = vec![vol("Vault", "U1", Some("abc"))];
        let e = diff(
            &Snapshot::default(),
            &snapshot(std::slice::from_ref(&a), &v),
            std::slice::from_ref(&a),
            &v,
        );
        assert!(
            e.iter().all(|e| !matches!(e, Event::Reconnected { .. })),
            "{e:?}"
        );
    }
}
