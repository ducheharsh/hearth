//! External drive detection and management.
//!
//! * macOS: every real directory in `/Volumes` + `diskutil info -plist`.
//! * Linux: `lsblk --json`, hot-pluggable/removable filesystems that are mounted.
//!
//! Parsers are platform-independent (and unit-tested everywhere); only the
//! command that feeds them is chosen per platform.

use std::path::{Path, PathBuf};

use hearth_core::storage::{MARKER_FILE, Marker, Volume};
use serde::Deserialize;

use crate::{CommandError, run};

#[derive(Debug, thiserror::Error)]
pub enum VolumeError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("cannot parse disk info: {0}")]
    Parse(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not supported on this platform")]
    Unsupported,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> VolumeError + '_ {
    move |source| VolumeError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// All mounted external volumes, with markers read.
pub async fn list() -> Result<Vec<Volume>, VolumeError> {
    let mut vols = if cfg!(target_os = "macos") {
        list_macos().await?
    } else if cfg!(target_os = "linux") {
        let raw = run(
            "lsblk",
            &[
                "--json",
                "--bytes",
                "-o",
                "NAME,UUID,FSTYPE,MOUNTPOINT,SIZE,FSAVAIL,RM,HOTPLUG,LABEL,TRAN,RO",
            ],
            &[],
        )
        .await?;
        parse_lsblk(&raw)?
    } else {
        return Err(VolumeError::Unsupported);
    };
    for v in &mut vols {
        v.marker = read_marker(&v.mount).await;
    }
    vols.sort_by(|a, b| a.mount.cmp(&b.mount));
    Ok(vols)
}

/// Cheap fingerprint of what's mounted; the watcher only does a full scan when it changes.
pub async fn mount_fingerprint() -> String {
    if cfg!(target_os = "macos") {
        let mut names = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir("/Volumes").await {
            while let Ok(Some(e)) = rd.next_entry().await {
                names.push(e.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort();
        names.join("\n")
    } else {
        tokio::fs::read_to_string("/proc/mounts")
            .await
            .unwrap_or_default()
    }
}

async fn list_macos() -> Result<Vec<Volume>, VolumeError> {
    let mut out = Vec::new();
    let mut rd = tokio::fs::read_dir("/Volumes")
        .await
        .map_err(io(Path::new("/Volumes")))?;
    while let Ok(Some(entry)) = rd.next_entry().await {
        // Skip the "Macintosh HD -> /" symlink and any other links.
        let Ok(ft) = entry.file_type().await else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let path = entry.path();
        let path_s = path.to_string_lossy().into_owned();
        let Ok(raw) = run_bytes("diskutil", &["info", "-plist", &path_s]).await else {
            continue; // not a mount point (stale dir), ignore
        };
        match parse_diskutil(&raw) {
            Ok(Some(v)) => out.push(v),
            Ok(None) => {}
            Err(e) => tracing::warn!(mount = %path_s, error = %e, "skipping volume"),
        }
    }
    Ok(out)
}

async fn run_bytes(program: &str, args: &[&str]) -> Result<Vec<u8>, CommandError> {
    let out = tokio::process::Command::new(program)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|source| CommandError::Spawn {
            program: program.to_owned(),
            source,
        })?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(CommandError::Failed {
            command: format!("{program} {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DiskutilInfo {
    volume_name: Option<String>,
    #[serde(rename = "VolumeUUID")]
    volume_uuid: Option<String>,
    filesystem_type: Option<String>,
    bus_protocol: Option<String>,
    internal: Option<bool>,
    ejectable: Option<bool>,
    writable_volume: Option<bool>,
    total_size: Option<u64>,
    free_space: Option<u64>,
    #[serde(rename = "APFSContainerFree")]
    apfs_container_free: Option<u64>,
    device_identifier: Option<String>,
    mount_point: Option<String>,
}

/// Parse `diskutil info -plist`. `Ok(None)` for internal or unmounted volumes.
pub fn parse_diskutil(raw: &[u8]) -> Result<Option<Volume>, VolumeError> {
    let i: DiskutilInfo = plist::from_bytes(raw).map_err(|e| VolumeError::Parse(e.to_string()))?;
    let Some(mount) = i.mount_point.filter(|m| !m.is_empty()) else {
        return Ok(None);
    };
    if i.internal.unwrap_or(false) {
        return Ok(None);
    }
    // APFS reports FreeSpace=0; the container's free space is the real number.
    let free = match (i.apfs_container_free, i.free_space) {
        (Some(c), _) if c > 0 => c,
        (_, Some(f)) => f,
        _ => 0,
    };
    let name = i
        .volume_name
        .unwrap_or_else(|| mount.rsplit('/').next().unwrap_or("Drive").to_owned());
    Ok(Some(Volume {
        mount: PathBuf::from(mount),
        name,
        volume_uuid: i.volume_uuid,
        filesystem: i.filesystem_type.unwrap_or_else(|| "unknown".into()),
        protocol: i.bus_protocol,
        device: i.device_identifier.unwrap_or_default(),
        writable: i.writable_volume.unwrap_or(false),
        ejectable: i.ejectable.unwrap_or(false),
        total: i.total_size.unwrap_or(0),
        free,
        marker: None,
    }))
}

#[derive(Debug, Deserialize)]
struct Lsblk {
    blockdevices: Vec<LsblkDev>,
}

#[derive(Debug, Deserialize)]
struct LsblkDev {
    name: String,
    uuid: Option<String>,
    fstype: Option<String>,
    mountpoint: Option<String>,
    size: Option<u64>,
    fsavail: Option<u64>,
    #[serde(default)]
    rm: bool,
    #[serde(default)]
    hotplug: bool,
    label: Option<String>,
    tran: Option<String>,
    #[serde(default)]
    ro: bool,
    #[serde(default)]
    children: Vec<LsblkDev>,
}

/// Parse `lsblk --json --bytes`; keeps mounted removable/hot-plug filesystems.
pub fn parse_lsblk(raw: &str) -> Result<Vec<Volume>, VolumeError> {
    fn walk(d: &LsblkDev, external: bool, tran: Option<&str>, out: &mut Vec<Volume>) {
        let external = external || d.rm || d.hotplug;
        let tran = d.tran.as_deref().or(tran);
        if let (true, Some(mp), Some(fs)) = (external, d.mountpoint.as_deref(), d.fstype.as_deref())
            && mp != "[SWAP]"
        {
            out.push(Volume {
                mount: PathBuf::from(mp),
                name: d.label.clone().unwrap_or_else(|| d.name.clone()),
                volume_uuid: d.uuid.clone(),
                filesystem: fs.to_owned(),
                protocol: tran.map(str::to_uppercase),
                device: d.name.clone(),
                writable: !d.ro,
                ejectable: true,
                total: d.size.unwrap_or(0),
                free: d.fsavail.unwrap_or(0),
                marker: None,
            });
        }
        for c in &d.children {
            walk(c, external, tran, out);
        }
    }
    let parsed: Lsblk = serde_json::from_str(raw).map_err(|e| VolumeError::Parse(e.to_string()))?;
    let mut out = Vec::new();
    for d in &parsed.blockdevices {
        walk(d, false, None, &mut out);
    }
    Ok(out)
}

/// Read `.hearth-volume.json` from a drive root, ignoring anything malformed.
pub async fn read_marker(mount: &Path) -> Option<Marker> {
    let raw = tokio::fs::read_to_string(mount.join(MARKER_FILE))
        .await
        .ok()?;
    serde_json::from_str(&raw).ok()
}

/// Write the marker atomically (temp file + rename) on the drive.
pub async fn write_marker(mount: &Path, marker: &Marker) -> Result<(), VolumeError> {
    let path = mount.join(MARKER_FILE);
    let tmp = mount.join(format!("{MARKER_FILE}.tmp"));
    let body = serde_json::to_vec_pretty(marker).map_err(|e| VolumeError::Parse(e.to_string()))?;
    tokio::fs::write(&tmp, body).await.map_err(io(&tmp))?;
    tokio::fs::rename(&tmp, &path).await.map_err(io(&path))
}

/// Safely eject (unmount + power down where supported).
pub async fn eject(v: &Volume) -> Result<(), VolumeError> {
    if cfg!(target_os = "macos") {
        let mount = v.mount.to_string_lossy();
        run("diskutil", &["eject", &mount], &[]).await?;
        Ok(())
    } else if cfg!(target_os = "linux") {
        let dev = format!("/dev/{}", v.device);
        run("udisksctl", &["unmount", "-b", &dev], &[]).await?;
        Ok(())
    } else {
        Err(VolumeError::Unsupported)
    }
}

/// Where a role link points while its drive is offline. Nothing (not even root)
/// can create directories under `/dev/null`, so containers fail to start instead of
/// silently writing to the internal disk.
pub const OFFLINE_TARGET: &str = "/dev/null/hearth-volume-offline";

/// Atomically point `link` at `target` (create or replace).
pub fn point_link(link: &Path, target: &Path) -> Result<(), VolumeError> {
    if std::fs::read_link(link).is_ok_and(|cur| cur == target) {
        return Ok(());
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    if link.exists() && std::fs::symlink_metadata(link).is_ok_and(|m| !m.file_type().is_symlink()) {
        // Never clobber a real directory someone created by hand.
        return Err(VolumeError::Io {
            path: link.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "exists and is not a symlink",
            ),
        });
    }
    let tmp = link.with_extension("tmp-link");
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(target, &tmp).map_err(io(&tmp))?;
    std::fs::rename(&tmp, link).map_err(io(link))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKUTIL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>APFSContainerFree</key><integer>66252800</integer>
<key>BusProtocol</key><string>USB</string>
<key>DeviceIdentifier</key><string>disk5s1</string>
<key>Ejectable</key><true/>
<key>FilesystemType</key><string>apfs</string>
<key>FreeSpace</key><integer>0</integer>
<key>Internal</key><false/>
<key>MountPoint</key><string>/Volumes/Vault 1</string>
<key>TotalSize</key><integer>67067904</integer>
<key>VolumeName</key><string>Vault</string>
<key>VolumeUUID</key><string>58787747-1DED-4680-BEDD-E11DBF411C70</string>
<key>WritableVolume</key><true/>
<key>SomethingElse</key><string>ignored</string>
</dict></plist>"#;

    #[test]
    fn parses_external_apfs_volume() {
        let v = parse_diskutil(DISKUTIL.as_bytes())
            .expect("parses")
            .expect("external");
        assert_eq!(v.mount, PathBuf::from("/Volumes/Vault 1"));
        assert_eq!(v.name, "Vault");
        assert_eq!(v.free, 66_252_800, "uses APFS container free space");
        assert_eq!(v.protocol.as_deref(), Some("USB"));
        assert!(v.writable && v.ejectable);
    }

    #[test]
    fn skips_internal_volumes() {
        let raw = DISKUTIL.replace("<key>Internal</key><false/>", "<key>Internal</key><true/>");
        assert!(parse_diskutil(raw.as_bytes()).expect("parses").is_none());
    }

    #[test]
    fn parses_lsblk_nested_usb_partition() {
        let raw = r#"{"blockdevices":[
          {"name":"nvme0n1","uuid":null,"fstype":null,"mountpoint":null,"size":512000000000,"fsavail":null,"rm":false,"hotplug":false,"label":null,"tran":"nvme","ro":false,
           "children":[{"name":"nvme0n1p1","uuid":"root","fstype":"ext4","mountpoint":"/","size":500000000000,"fsavail":1,"rm":false,"hotplug":false,"label":null,"tran":null,"ro":false}]},
          {"name":"sda","uuid":null,"fstype":null,"mountpoint":null,"size":1000000000000,"fsavail":null,"rm":false,"hotplug":true,"label":null,"tran":"usb","ro":false,
           "children":[{"name":"sda1","uuid":"U-1","fstype":"ext4","mountpoint":"/media/vault","size":999000000000,"fsavail":800000000000,"rm":false,"hotplug":true,"label":"Vault","tran":null,"ro":false}]}
        ]}"#;
        let v = parse_lsblk(raw).expect("parses");
        assert_eq!(v.len(), 1, "internal root disk excluded");
        let v = &v[0];
        assert_eq!(v.name, "Vault");
        assert_eq!(
            v.protocol.as_deref(),
            Some("USB"),
            "inherits transport from parent"
        );
        assert_eq!(v.volume_uuid.as_deref(), Some("U-1"));
    }

    #[tokio::test]
    async fn marker_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let m = Marker {
            version: 1,
            id: "abc".into(),
            role: hearth_core::storage::Role::Vault,
            label: "Vault".into(),
            host: "m1".into(),
            attached_at: 1,
        };
        write_marker(dir.path(), &m).await.expect("writes");
        assert_eq!(read_marker(dir.path()).await, Some(m));
        tokio::fs::write(dir.path().join(MARKER_FILE), "garbage")
            .await
            .expect("write");
        assert_eq!(read_marker(dir.path()).await, None);
    }

    #[test]
    fn link_points_and_repoints_atomically_and_refuses_real_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let link = dir.path().join("volumes/vault");
        point_link(&link, Path::new("/Volumes/A")).expect("create");
        assert_eq!(
            std::fs::read_link(&link).expect("link"),
            PathBuf::from("/Volumes/A")
        );
        point_link(&link, Path::new(OFFLINE_TARGET)).expect("repoint");
        assert_eq!(
            std::fs::read_link(&link).expect("link"),
            PathBuf::from(OFFLINE_TARGET)
        );
        assert!(
            std::fs::create_dir_all(link.join("photos")).is_err(),
            "offline target must not be creatable"
        );

        let real = dir.path().join("volumes/media");
        std::fs::create_dir_all(&real).expect("mkdir");
        assert!(point_link(&real, Path::new("/x")).is_err());
    }
}
