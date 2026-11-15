//! The "Discover" directory: every app from awesome-selfhosted-data
//! (<https://github.com/awesome-selfhosted/awesome-selfhosted-data>, CC BY-SA 3.0).
//!
//! Pure parsing, search and ranking. Fetching the data lives in `hearth-host`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const ATTRIBUTION: &str =
    "Directory data from awesome-selfhosted (CC BY-SA 3.0), github.com/awesome-selfhosted";

#[derive(Debug, Deserialize)]
struct RawRelease {
    tag: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSoftware {
    name: String,
    #[serde(default)]
    website_url: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    licenses: Vec<String>,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source_code_url: Option<String>,
    #[serde(default)]
    demo_url: Option<String>,
    #[serde(default)]
    stargazers_count: Option<u64>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    current_release: Option<RawRelease>,
    #[serde(default)]
    commit_history: BTreeMap<String, u32>,
    #[serde(default)]
    depends_3rdparty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    pub tag: String,
    pub published_at: Option<String>,
}

/// One directory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// File stem in the data repo; stable id.
    pub slug: String,
    pub name: String,
    pub description: String,
    pub website: Option<String>,
    pub source: Option<String>,
    pub demo: Option<String>,
    /// `owner/name` when the source lives on GitHub.
    pub repo: Option<String>,
    pub licenses: Vec<String>,
    pub platforms: Vec<String>,
    pub tags: Vec<String>,
    pub stars: Option<u64>,
    pub updated_at: Option<String>,
    pub archived: bool,
    pub release: Option<Release>,
    /// `YYYY-MM` → commits that month.
    pub commits: BTreeMap<String, u32>,
    /// Needs a third-party service to be useful.
    pub depends_3rdparty: bool,
    pub has_icon: bool,
}

impl Entry {
    #[must_use]
    pub fn docker(&self) -> bool {
        self.platforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("docker"))
    }

    #[must_use]
    pub fn commits_in(&self, month: &str) -> u32 {
        self.commits.get(month).copied().unwrap_or(0)
    }
}

/// Parse one `software/<slug>.yml` file.
pub fn parse_entry(slug: &str, yaml: &str, has_icon: bool) -> Result<Entry, serde_yaml_ng::Error> {
    let r: RawSoftware = serde_yaml_ng::from_str(yaml)?;
    Ok(Entry {
        slug: slug.to_owned(),
        repo: r.source_code_url.as_deref().and_then(github_repo),
        name: r.name,
        description: r.description,
        website: r.website_url,
        source: r.source_code_url,
        demo: r.demo_url,
        licenses: r.licenses,
        platforms: r.platforms,
        tags: r.tags,
        stars: r.stargazers_count,
        updated_at: r.updated_at,
        archived: r.archived,
        release: r.current_release.and_then(|c| {
            c.tag.map(|tag| Release {
                tag,
                published_at: c.published_at,
            })
        }),
        commits: r.commit_history,
        depends_3rdparty: r.depends_3rdparty,
        has_icon,
    })
}

/// `https://github.com/owner/name(.git)(/...)` → `owner/name` (lowercased).
#[must_use]
pub fn github_repo(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let name = parts.next()?.trim_end_matches(".git");
    Some(format!("{owner}/{name}").to_ascii_lowercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Stars,
    Updated,
    Name,
    Activity,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Query {
    pub q: String,
    pub tag: Option<String>,
    pub docker_only: bool,
    pub include_archived: bool,
    pub sort: Sort,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Serialize)]
pub struct Page<'a> {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub items: Vec<&'a Entry>,
}

/// The latest month key present in the data (e.g. `2026-11`).
#[must_use]
pub fn latest_month(entries: &[Entry]) -> Option<String> {
    entries
        .iter()
        .filter_map(|e| e.commits.keys().next_back())
        .max()
        .cloned()
}

/// Filter, sort and paginate.
#[must_use]
pub fn search<'a>(entries: &'a [Entry], q: &Query) -> Page<'a> {
    let needle = q.q.trim().to_lowercase();
    let month = latest_month(entries).unwrap_or_default();
    let mut hits: Vec<&Entry> = entries
        .iter()
        .filter(|e| q.include_archived || !e.archived)
        .filter(|e| !q.docker_only || e.docker())
        .filter(|e| q.tag.as_ref().is_none_or(|t| e.tags.iter().any(|x| x == t)))
        .filter(|e| {
            needle.is_empty()
                || e.name.to_lowercase().contains(&needle)
                || e.description.to_lowercase().contains(&needle)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&needle))
        })
        .collect();
    match q.sort {
        Sort::Stars => hits.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name))),
        Sort::Updated => hits.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        Sort::Name => hits.sort_by_key(|e| e.name.to_lowercase()),
        Sort::Activity => hits.sort_by_key(|e| std::cmp::Reverse(e.commits_in(&month))),
    }
    let per_page = q.per_page.clamp(1, 100);
    let total = hits.len();
    let items = hits
        .into_iter()
        .skip(q.page * per_page)
        .take(per_page)
        .collect();
    Page {
        total,
        page: q.page,
        per_page,
        items,
    }
}

/// Ranked lists for the marketplace front page.
#[derive(Debug, Serialize)]
pub struct Leaderboards<'a> {
    /// Month the activity ranking is based on (`YYYY-MM`).
    pub month: String,
    /// Hottest this month: commits this month weighted by popularity.
    pub top_month: Vec<&'a Entry>,
    pub most_starred: Vec<&'a Entry>,
    /// Smaller projects (< 5k stars) with the most activity over the last 3 months.
    pub rising: Vec<&'a Entry>,
    /// Most recent releases.
    pub fresh_releases: Vec<&'a Entry>,
}

#[allow(clippy::cast_precision_loss)] // ranking only
fn momentum(e: &Entry, month: &str) -> f64 {
    f64::from(e.commits_in(month)) * ((e.stars.unwrap_or(0) as f64) + 10.0).log10()
}

#[must_use]
pub fn leaderboards(entries: &[Entry], n: usize) -> Leaderboards<'_> {
    let month = latest_month(entries).unwrap_or_default();
    let pool: Vec<&Entry> = entries
        .iter()
        .filter(|e| !e.archived && e.docker())
        .collect();

    let mut top = pool.clone();
    top.sort_by(|a, b| momentum(b, &month).total_cmp(&momentum(a, &month)));

    let mut starred = pool.clone();
    starred.sort_by_key(|e| std::cmp::Reverse(e.stars));

    let recent: Vec<String> = {
        let mut keys: Vec<&String> = entries.iter().flat_map(|e| e.commits.keys()).collect();
        keys.sort();
        keys.dedup();
        keys.into_iter().rev().take(3).cloned().collect()
    };
    let three = |e: &Entry| recent.iter().map(|m| e.commits_in(m)).sum::<u32>();
    let mut rising: Vec<&Entry> = pool
        .iter()
        .copied()
        .filter(|e| e.stars.is_some_and(|s| (100..5_000).contains(&s)))
        .collect();
    rising.sort_by_key(|e| std::cmp::Reverse(three(e)));

    let mut fresh: Vec<&Entry> = pool
        .iter()
        .copied()
        .filter(|e| e.release.is_some())
        .collect();
    fresh.sort_by(|a, b| {
        let d = |e: &Entry| e.release.as_ref().and_then(|r| r.published_at.clone());
        d(b).cmp(&d(a))
    });

    Leaderboards {
        month,
        top_month: top.into_iter().take(n).collect(),
        most_starred: starred.into_iter().take(n).collect(),
        rising: rising.into_iter().take(n).collect(),
        fresh_releases: fresh.into_iter().take(n).collect(),
    }
}

/// Tag → number of (non-archived) apps.
#[must_use]
pub fn tag_counts(entries: &[Entry]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for e in entries.iter().filter(|e| !e.archived) {
        for t in &e.tags {
            *m.entry(t.clone()).or_default() += 1;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMMICH: &str = "name: Immich
website_url: https://immich.app/
description: Photo and video backup.
licenses:
  - AGPL-3.0
platforms:
  - Docker
tags:
  - Photo Galleries
source_code_url: https://github.com/immich-app/immich
stargazers_count: 118657
updated_at: '2026-11-09'
archived: false
current_release:
  tag: v3.4.0
  published_at: '2026-11-06'
commit_history:
  2026-10: 287
  2026-11: 114
some_future_field: ignored
";

    fn entry(slug: &str, stars: u64, nov: u32, docker: bool) -> Entry {
        let mut e = parse_entry(slug, IMMICH, false).expect("parses");
        e.slug = slug.into();
        e.name = slug.into();
        e.stars = Some(stars);
        e.commits = BTreeMap::from([("2026-11".into(), nov)]);
        if !docker {
            e.platforms = vec!["Python".into()];
        }
        e
    }

    #[test]
    fn parses_real_entry_and_tolerates_new_fields() {
        let e = parse_entry("immich", IMMICH, true).expect("parses");
        assert_eq!(e.repo.as_deref(), Some("immich-app/immich"));
        assert_eq!(e.stars, Some(118_657));
        assert!(e.docker());
        assert_eq!(e.commits_in("2026-10"), 287);
        assert_eq!(e.release.as_ref().map(|r| r.tag.as_str()), Some("v3.4.0"));
    }

    #[test]
    fn github_repo_parsing() {
        assert_eq!(
            github_repo("https://github.com/Foo/Bar.git"),
            Some("foo/bar".into())
        );
        assert_eq!(
            github_repo("https://github.com/foo/bar/tree/main"),
            Some("foo/bar".into())
        );
        assert_eq!(github_repo("https://gitlab.com/foo/bar"), None);
        assert_eq!(github_repo("https://github.com/onlyowner"), None);
    }

    #[test]
    fn search_filters_sorts_and_paginates() {
        let es = vec![
            entry("a", 10, 1, true),
            entry("b", 300, 5, true),
            entry("c", 50, 9, false),
        ];
        let p = search(
            &es,
            &Query {
                per_page: 2,
                ..Query::default()
            },
        );
        assert_eq!(p.total, 3);
        assert_eq!(
            p.items.iter().map(|e| e.slug.as_str()).collect::<Vec<_>>(),
            ["b", "c"]
        );
        let p = search(
            &es,
            &Query {
                docker_only: true,
                sort: Sort::Activity,
                per_page: 10,
                ..Query::default()
            },
        );
        assert_eq!(
            p.items.iter().map(|e| e.slug.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        let p = search(
            &es,
            &Query {
                q: "PHOTO".into(),
                per_page: 10,
                ..Query::default()
            },
        );
        assert_eq!(p.total, 3, "matches description/tags case-insensitively");
    }

    #[test]
    fn leaderboards_rank_by_momentum_and_skip_non_docker() {
        let es = vec![
            entry("busy-small", 200, 100, true),
            entry("huge-quiet", 100_000, 1, true),
            entry("busy-big", 50_000, 80, true),
            entry("no-docker", 90_000, 500, false),
        ];
        let lb = leaderboards(&es, 5);
        assert_eq!(lb.month, "2026-11");
        assert_eq!(lb.top_month[0].slug, "busy-big");
        assert!(lb.top_month.iter().all(|e| e.slug != "no-docker"));
        assert_eq!(lb.most_starred[0].slug, "huge-quiet");
        assert_eq!(
            lb.rising
                .iter()
                .map(|e| e.slug.as_str())
                .collect::<Vec<_>>(),
            ["busy-small"]
        );
    }
}
