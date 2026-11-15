//! The curated catalog (`catalog/`: manifests, packs, logos) compiled into the binary,
//! so a packaged hearth (Homebrew, the macOS app) needs nothing next to it.
//!
//! If `catalog_dir` has no manifests, the embedded copy is written to
//! `<state_dir>/catalog` on every start (so upgrades bring new apps) and used instead.

use std::path::Path;

use anyhow::Context as _;
use rust_embed::Embed;

use crate::config::Config;

#[derive(Embed)]
#[folder = "../../catalog"]
struct Catalog;

fn has_manifests(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|rd| {
        rd.filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "yml"))
    })
}

/// Point `config.catalog_dir` at a usable catalog, unpacking the embedded one if needed.
pub fn ensure_catalog(config: &mut Config) -> anyhow::Result<()> {
    if has_manifests(&config.catalog_dir) {
        return Ok(());
    }
    let dir = config.state_dir.join("catalog");
    // Start clean so apps or packs removed upstream disappear after an upgrade.
    for sub in [dir.clone(), dir.join("packs")] {
        if let Ok(rd) = std::fs::read_dir(&sub) {
            for e in rd
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "yml"))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let mut n = 0;
    for name in Catalog::iter() {
        let file = Catalog::get(&name).context("embedded catalog file vanished")?;
        let path = dir.join(name.as_ref());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, file.data.as_ref())
            .with_context(|| format!("writing {}", path.display()))?;
        n += 1;
    }
    tracing::info!(files = n, dir = %dir.display(), "using the built-in catalog");
    config.catalog_dir = dir;
    Ok(())
}
