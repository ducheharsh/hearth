//! Packs: curated stacks that set up a whole use case in one go
//! ("Local AI", "Media Server", …).
//!
//! A pack lists catalog apps (some optional) plus native pieces that must run on
//! the Mac itself (Ollama, for GPU access). [`plan`] works out what is already
//! there, what will be installed, and the combined resources to check capacity.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;
use crate::manifest::Resources;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub id: String,
    pub name: String,
    /// One line for cards.
    pub tagline: String,
    pub description: String,
    /// Phosphor icon name used by the UI (`brain`, `film-strip`, …).
    pub icon: String,
    pub apps: Vec<PackApp>,
    #[serde(default)]
    pub ollama: Option<OllamaSpec>,
    /// Short "after install" tips.
    #[serde(default)]
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackApp {
    pub id: String,
    /// Why this app is in the pack.
    pub role: String,
    #[serde(default)]
    pub optional: bool,
}

/// Ollama runs natively (Metal GPU); Docker on macOS can't use the GPU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OllamaSpec {
    pub models: Vec<Model>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub role: String,
    /// Download size in MiB.
    pub size_mb: u64,
    /// RAM while loaded, in MiB.
    pub ram_mb: u64,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("invalid pack: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("pack `{pack}` uses unknown app `{app}`")]
    UnknownApp { pack: String, app: String },
    #[error("pack `{0}` is empty")]
    Empty(String),
}

impl Pack {
    pub fn from_yaml(src: &str) -> Result<Self, PackError> {
        Ok(serde_yaml_ng::from_str(src)?)
    }

    /// Every app must exist in the catalog.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), PackError> {
        if self.apps.is_empty() && self.ollama.is_none() {
            return Err(PackError::Empty(self.id.clone()));
        }
        for a in &self.apps {
            if catalog.get(&a.id).is_none() {
                return Err(PackError::UnknownApp {
                    pack: self.id.clone(),
                    app: a.id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberState {
    Installed,
    Install,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedApp {
    pub id: String,
    pub name: String,
    pub role: String,
    pub optional: bool,
    pub ram_mb: u64,
    pub state: MemberState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedModel {
    pub name: String,
    pub role: String,
    pub optional: bool,
    pub size_mb: u64,
    pub ram_mb: u64,
    pub state: MemberState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackPlan {
    pub apps: Vec<PlannedApp>,
    pub models: Vec<PlannedModel>,
    pub needs_ollama: bool,
    /// What the new pieces add (apps + the largest model, since Ollama loads one at a time).
    pub added: Resources,
    /// Total download for models, MiB.
    pub download_mb: u64,
    /// Everything already in place.
    pub complete: bool,
}

/// What installing `pack` would do. `with` names the optional apps/models to include;
/// `installed_apps`/`installed_models` describe the machine now.
#[must_use]
pub fn plan(
    pack: &Pack,
    catalog: &Catalog,
    with: &BTreeSet<String>,
    installed_apps: &BTreeSet<String>,
    installed_models: &BTreeSet<String>,
) -> PackPlan {
    let state = |name: &str, optional: bool, have: bool| {
        if have {
            MemberState::Installed
        } else if optional && !with.contains(name) {
            MemberState::Skip
        } else {
            MemberState::Install
        }
    };
    let mut added = Resources::default();
    let apps: Vec<PlannedApp> = pack
        .apps
        .iter()
        .filter_map(|a| {
            let m = catalog.get(&a.id)?;
            let s = state(&a.id, a.optional, installed_apps.contains(&a.id));
            if s == MemberState::Install {
                added.ram_mb += m.resources.ram_mb;
                added.vault_gb += m.resources.vault_gb;
                added.needs_vault |= m.resources.needs_vault;
            }
            Some(PlannedApp {
                id: a.id.clone(),
                name: m.name.clone(),
                role: a.role.clone(),
                optional: a.optional,
                ram_mb: m.resources.ram_mb,
                state: s,
            })
        })
        .collect();
    let models: Vec<PlannedModel> = pack
        .ollama
        .iter()
        .flat_map(|o| &o.models)
        .map(|m| PlannedModel {
            name: m.name.clone(),
            role: m.role.clone(),
            optional: m.optional,
            size_mb: m.size_mb,
            ram_mb: m.ram_mb,
            state: state(&m.name, m.optional, has_model(installed_models, &m.name)),
        })
        .collect();
    let new_models = models.iter().filter(|m| m.state == MemberState::Install);
    added.ram_mb += new_models.clone().map(|m| m.ram_mb).max().unwrap_or(0);
    let download_mb = new_models.map(|m| m.size_mb).sum();
    let complete = apps.iter().all(|a| a.state != MemberState::Install)
        && models.iter().all(|m| m.state != MemberState::Install);
    PackPlan {
        needs_ollama: pack.ollama.is_some(),
        apps,
        models,
        added,
        download_mb,
        complete,
    }
}

/// Ollama reports `llama3.2:3b`; a bare name means `:latest`.
fn has_model(installed: &BTreeSet<String>, name: &str) -> bool {
    let full = if name.contains(':') {
        name.to_owned()
    } else {
        format!("{name}:latest")
    };
    installed.contains(name) || installed.contains(&full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AppManifest;

    fn app(id: &str, port: u16, ram: u64) -> AppManifest {
        AppManifest::from_yaml(&format!(
            "id: {id}\nname: {id}\ndescription: d\ncategory: ai\nport: {port}\nupstream: {{ service: s, port: 80 }}\nresources: {{ ram_mb: {ram} }}\ncompose: |\n  services:\n    s:\n      image: nginx\n"
        ))
        .expect("manifest")
    }

    fn fixture() -> (Pack, Catalog) {
        let pack = Pack::from_yaml(
            "id: ai\nname: Local AI\ntagline: t\ndescription: d\nicon: brain\napps:\n  - { id: chat, role: Chat }\n  - { id: flows, role: Agents, optional: true }\nollama:\n  models:\n    - { name: 'llama3.2:3b', role: Chat model, size_mb: 2000, ram_mb: 2600 }\n    - { name: nomic-embed-text, role: Embeddings, size_mb: 270, ram_mb: 350 }\n    - { name: 'qwen2.5-coder:7b', role: Code, size_mb: 4700, ram_mb: 5200, optional: true }\n",
        )
        .expect("pack");
        let catalog = Catalog::from_manifests([app("chat", 3004, 600), app("flows", 5678, 400)])
            .expect("catalog");
        (pack, catalog)
    }

    fn set(xs: &[&str]) -> BTreeSet<String> {
        xs.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn fresh_machine_installs_required_parts_only() {
        let (pack, cat) = fixture();
        pack.validate(&cat).expect("valid");
        let p = plan(&pack, &cat, &set(&[]), &set(&[]), &set(&[]));
        let states: Vec<_> = p.apps.iter().map(|a| a.state).collect();
        assert_eq!(states, [MemberState::Install, MemberState::Skip]);
        // 600 MB app + the biggest model that will be loaded (2600), not the sum.
        assert_eq!(p.added.ram_mb, 3200);
        assert_eq!(p.download_mb, 2270);
        assert!(!p.complete);
    }

    #[test]
    fn opting_in_and_already_installed() {
        let (pack, cat) = fixture();
        let p = plan(
            &pack,
            &cat,
            &set(&["flows"]),
            &set(&["chat"]),
            &set(&["llama3.2:3b", "nomic-embed-text:latest"]),
        );
        assert_eq!(p.apps[0].state, MemberState::Installed);
        assert_eq!(p.apps[1].state, MemberState::Install);
        assert_eq!(p.added.ram_mb, 400);
        assert_eq!(p.download_mb, 0);
        let done = plan(
            &pack,
            &cat,
            &set(&[]),
            &set(&["chat"]),
            &set(&["llama3.2:3b", "nomic-embed-text"]),
        );
        assert!(done.complete);
    }

    #[test]
    fn rejects_unknown_apps() {
        let (mut pack, cat) = fixture();
        pack.apps.push(PackApp {
            id: "nope".into(),
            role: "x".into(),
            optional: false,
        });
        assert!(matches!(
            pack.validate(&cat),
            Err(PackError::UnknownApp { .. })
        ));
    }
}
