//! What to back up for an app, and how long to keep it.
//!
//! An app's backup = its whole hearth app directory (`data/`, config, secret) plus every
//! bind mount under a storage role it uses (`${VAULT}/photos`, …). Media libraries are
//! excluded by default (huge, usually re-downloadable); backups never back up backups.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compose_config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Retention {
    pub daily: u32,
    pub weekly: u32,
    pub monthly: u32,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            daily: 7,
            weekly: 4,
            monthly: 6,
        }
    }
}

/// Replace `${VAR}` occurrences using `vars`; unknown variables are left as-is.
#[must_use]
pub fn expand(s: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = s.to_owned();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Host paths to back up for one app.
///
/// `vars` must contain the role variables (`VAULT`, `MEDIA`, `BACKUPS`) and `APP_DATA`.
#[must_use]
pub fn paths_for(
    app_dir: &Path,
    compose: &str,
    vars: &BTreeMap<String, String>,
    include_media: bool,
) -> Vec<PathBuf> {
    let mut out = vec![app_dir.to_path_buf()];
    let roots: Vec<(&str, bool)> = vec![
        ("VAULT", true),
        ("MEDIA", include_media),
        ("BACKUPS", false),
    ];
    let Ok(services) = compose_config::extract(compose) else {
        return out;
    };
    for v in services.iter().flat_map(|s| s.volumes.iter()) {
        let source = v.split(':').next().unwrap_or_default();
        let Some((role, wanted)) = roots
            .iter()
            .find(|(r, _)| source.starts_with(&format!("${{{r}}}")))
        else {
            continue;
        };
        if !wanted || !vars.contains_key(*role) {
            continue;
        }
        let p = PathBuf::from(expand(source, vars));
        if !out.iter().any(|existing| p.starts_with(existing)) {
            out.retain(|existing| !existing.starts_with(&p));
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSE: &str = "services:
  app:
    image: x
    volumes:
      - ${APP_DATA}/db:/db
      - ${VAULT}/photos:/photos
      - ${VAULT}/photos/thumbs:/thumbs
      - ${MEDIA}:/media
      - ${BACKUPS}/dumps:/dumps
      - /etc/localtime:/etc/localtime:ro
";

    fn vars() -> BTreeMap<String, String> {
        [
            ("APP_DATA", "/s/apps/x/data"),
            ("VAULT", "/s/volumes/vault"),
            ("MEDIA", "/s/volumes/media"),
            ("BACKUPS", "/s/volumes/backups"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    #[test]
    fn app_dir_plus_vault_mounts_deduplicated() {
        let p = paths_for(Path::new("/s/apps/x"), COMPOSE, &vars(), false);
        assert_eq!(
            p,
            vec![
                PathBuf::from("/s/apps/x"),
                PathBuf::from("/s/volumes/vault/photos")
            ]
        );
    }

    #[test]
    fn media_only_when_asked_and_never_backups_or_host_paths() {
        let p = paths_for(Path::new("/s/apps/x"), COMPOSE, &vars(), true);
        assert!(p.contains(&PathBuf::from("/s/volumes/media")));
        assert!(
            !p.iter()
                .any(|x| x.starts_with("/s/volumes/backups") || x.starts_with("/etc"))
        );
    }

    #[test]
    fn expand_leaves_unknown_vars() {
        assert_eq!(
            expand("${VAULT}/a/${NOPE}", &vars()),
            "/s/volumes/vault/a/${NOPE}"
        );
    }
}
