//! The marketplace catalog: a directory of `<id>.yml` manifests.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::manifest::{AppManifest, ManifestError};

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("cannot read catalog at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Manifest {
        path: PathBuf,
        #[source]
        source: ManifestError,
    },
    #[error("duplicate app id `{0}`")]
    DuplicateId(String),
    #[error("apps `{a}` and `{b}` both claim port {port}")]
    PortClash { a: String, b: String, port: u16 },
}

/// An immutable, validated set of manifests, ordered by id.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    apps: BTreeMap<String, AppManifest>,
}

impl Catalog {
    /// Load every `*.yml` / `*.yaml` file in `dir` (non-recursive).
    pub fn load_dir(dir: &Path) -> Result<Self, CatalogError> {
        let io_err = |source| CatalogError::Io {
            path: dir.to_path_buf(),
            source,
        };
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(io_err)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "yml" || ext == "yaml")
            })
            .collect();
        paths.sort();

        let manifests = paths
            .into_iter()
            .map(|path| {
                let src = std::fs::read_to_string(&path).map_err(|source| CatalogError::Io {
                    path: path.clone(),
                    source,
                })?;
                AppManifest::from_yaml(&src)
                    .map_err(|source| CatalogError::Manifest { path, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_manifests(manifests)
    }

    /// Build a catalog, rejecting duplicate ids and port clashes.
    pub fn from_manifests(
        manifests: impl IntoIterator<Item = AppManifest>,
    ) -> Result<Self, CatalogError> {
        let mut apps = BTreeMap::new();
        let mut ports: HashMap<u16, String> = HashMap::new();
        for m in manifests {
            if let Some(other) = ports.insert(m.port, m.id.clone()) {
                return Err(CatalogError::PortClash {
                    a: other,
                    b: m.id,
                    port: m.port,
                });
            }
            let id = m.id.clone();
            if apps.insert(id.clone(), m).is_some() {
                return Err(CatalogError::DuplicateId(id));
            }
        }
        Ok(Self { apps })
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AppManifest> {
        self.apps.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &AppManifest> {
        self.apps.values()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.apps.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, port: u16) -> String {
        format!(
            "id: {id}\nname: {id}\ndescription: d\ncategory: c\nport: {port}\n\
             upstream: {{ service: web, port: 80 }}\ncompose: |\n  services:\n    web:\n      image: nginx\n"
        )
    }

    #[test]
    fn loads_directory_sorted_and_ignores_other_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("b.yml"), manifest("bravo", 4001)).expect("write");
        std::fs::write(dir.path().join("a.yaml"), manifest("alpha", 4000)).expect("write");
        std::fs::write(dir.path().join("README.md"), "ignored").expect("write");

        let cat = Catalog::load_dir(dir.path()).expect("loads");
        let ids: Vec<_> = cat.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "bravo"]);
    }

    #[test]
    fn rejects_port_clash() {
        let a = AppManifest::from_yaml(&manifest("alpha", 4000)).expect("valid");
        let b = AppManifest::from_yaml(&manifest("bravo", 4000)).expect("valid");
        assert!(matches!(
            Catalog::from_manifests([a, b]),
            Err(CatalogError::PortClash { port: 4000, .. })
        ));
    }

    #[test]
    fn reports_which_file_is_broken() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("bad.yml"), "id: [").expect("write");
        let err = Catalog::load_dir(dir.path()).expect_err("must fail");
        assert!(err.to_string().contains("bad.yml"), "{err}");
    }
}
