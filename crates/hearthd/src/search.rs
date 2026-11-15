//! Global search (⌘K and app search): builds a Tantivy index from live state.

use std::time::Duration;

use hearth_search::{Document, SearchIndex};

use crate::state::SharedState;

/// Settings sections and actions, so ⌘K can jump anywhere.
const PLACES: &[(&str, &str, &str, &str)] = &[
    (
        "setting",
        "settings:setup",
        "Setup checklist",
        "progress onboarding getting started",
    ),
    (
        "setting",
        "settings:general",
        "General settings",
        "server name time zone",
    ),
    (
        "setting",
        "settings:network",
        "Network: Tailscale, NetBird, Headscale",
        "vpn tailnet wireguard mesh login hostname",
    ),
    (
        "setting",
        "settings:access",
        "Logins & gateway",
        "authelia password 2fa sso users security",
    ),
    (
        "setting",
        "settings:domains",
        "Domains & tunnels",
        "cloudflare funnel ngrok public domain dns",
    ),
    (
        "setting",
        "settings:capacity",
        "Capacity guard",
        "memory ram disk reserve limits",
    ),
    (
        "setting",
        "settings:advanced",
        "Advanced",
        "danger reset discover sync remote user",
    ),
    (
        "action",
        "tab:storage",
        "Attach a drive",
        "ssd external storage vault media backups",
    ),
    (
        "action",
        "view:discover",
        "Discover 1,300+ apps",
        "browse awesome selfhosted directory",
    ),
    (
        "action",
        "custom:new",
        "Add your own app",
        "docker image compose custom bring",
    ),
    (
        "action",
        "theme:toggle",
        "Toggle theme",
        "dark light mode appearance",
    ),
    (
        "action",
        "tab:containers",
        "Containers",
        "docker logs terminal shell",
    ),
];

async fn documents(st: &SharedState) -> Vec<Document> {
    let mut docs = Vec::new();
    for (kind, id, title, tags) in PLACES {
        docs.push(Document {
            kind: (*kind).into(),
            id: (*id).into(),
            title: (*title).into(),
            subtitle: String::new(),
            tags: (*tags).into(),
            body: String::new(),
            icon: None,
            popularity: 0,
        });
    }
    {
        let stars: std::collections::BTreeMap<String, u64> = st
            .discover
            .read()
            .await
            .entries
            .iter()
            .filter_map(|e| Some((e.repo.clone()?, e.stars?)))
            .collect();
        let icons = st.config.icons_dir();
        for a in st.catalog.read().await.iter() {
            let icon_file = a.icon_file();
            docs.push(Document {
                kind: "app".into(),
                id: a.id.clone(),
                title: a.name.clone(),
                subtitle: a.category.clone(),
                tags: a.category.clone(),
                body: format!(
                    "{} {}",
                    a.description,
                    a.editors_note.clone().unwrap_or_default()
                ),
                icon: icons
                    .join(&icon_file)
                    .exists()
                    .then(|| format!("/api/icons/{icon_file}")),
                popularity: a
                    .repo
                    .as_ref()
                    .and_then(|r| stars.get(&r.to_ascii_lowercase()).copied())
                    .unwrap_or(0),
            });
        }
    }
    for e in st
        .discover
        .read()
        .await
        .entries
        .iter()
        .filter(|e| !e.archived)
    {
        docs.push(Document {
            kind: "discover".into(),
            id: e.slug.clone(),
            title: e.name.clone(),
            subtitle: e.tags.join(" · "),
            tags: e.tags.join(" "),
            body: e.description.clone(),
            icon: e.has_icon.then(|| format!("/api/icons/{}.webp", e.slug)),
            popularity: e.stars.unwrap_or(0),
        });
    }
    if let Some(d) = &st.docker
        && let Ok(list) = d.containers().await
    {
        for c in list {
            docs.push(Document {
                kind: "container".into(),
                id: c.name.clone(),
                title: c.name.clone(),
                subtitle: format!("{} · {}", c.state, c.image),
                tags: c.project.clone().unwrap_or_default(),
                body: c.image,
                icon: None,
                popularity: 0,
            });
        }
    }
    for v in &st.storage.read().await.volumes {
        docs.push(Document {
            kind: "drive".into(),
            id: v.mount.display().to_string(),
            title: v.name.clone(),
            subtitle: v.mount.display().to_string(),
            tags: format!("{} drive disk ssd", v.filesystem),
            body: String::new(),
            icon: None,
            popularity: 0,
        });
    }
    docs
}

pub async fn rebuild(st: &SharedState) {
    let docs = documents(st).await;
    match tokio::task::spawn_blocking(move || SearchIndex::build(docs)).await {
        Ok(Ok(ix)) => *st.search.write().await = Some(ix),
        Ok(Err(e)) => tracing::warn!(error = %e, "search index build failed"),
        Err(e) => tracing::warn!(error = %e, "search index task failed"),
    }
}

pub async fn run_indexer(st: SharedState) {
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tick.tick().await;
        rebuild(&st).await;
    }
}
