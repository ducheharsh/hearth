//! Tiny JSON persistence: load-or-default, atomic owner-only writes.

use std::path::Path;

use anyhow::Context as _;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Load `path`, or `T::default()` if it doesn't exist. Corrupt files are an error
/// (never silently reset user state).
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> anyhow::Result<T> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Write `value` as pretty JSON via temp file + rename, mode 0600.
pub fn save<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn roundtrip_default_and_corruption() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("x/state.json");
        let empty: BTreeMap<String, u32> = load(&p).expect("default");
        assert!(empty.is_empty());
        save(&p, &BTreeMap::from([("a".to_owned(), 1u32)])).expect("save");
        let back: BTreeMap<String, u32> = load(&p).expect("load");
        assert_eq!(back.get("a"), Some(&1));
        std::fs::write(&p, "{oops").expect("write");
        assert!(
            load::<BTreeMap<String, u32>>(&p).is_err(),
            "corrupt state is an error, not a reset"
        );
    }
}
