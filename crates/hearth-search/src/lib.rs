//! In-process search over everything hearth knows: apps, the Discover directory,
//! containers, drives, settings and actions.
//!
//! Why Tantivy and not OpenSearch/Elasticsearch: the corpus is a few thousand small
//! documents on an 8 GB machine. Tantivy gives the same Lucene-style BM25 ranking with
//! zero extra services and ~ms indexing; a JVM search cluster would cost 1-2 GB of RAM.
//!
//! Matching per query token, across fields with boosts (title > tags > body):
//! exact term, prefix (search-as-you-type, last token), and 1-edit fuzzy (typos).
//! Every token must match somewhere (AND), then results are nudged by popularity.

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::{
    Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TEXT, Value as _,
};
use tantivy::{Index, IndexReader, TantivyDocument, Term, doc};

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("search index error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
}

/// Something that can be found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// `app`, `discover`, `container`, `drive`, `setting`, `action`
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    /// Extra keywords (categories, tags, synonyms).
    pub tags: String,
    pub body: String,
    /// Icon URL (optional).
    pub icon: Option<String>,
    /// Popularity signal (e.g. GitHub stars); 0 if unknown.
    pub popularity: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    #[serde(flatten)]
    pub doc: Document,
    pub score: f32,
}

struct Fields {
    kind: Field,
    title: Field,
    tags: Field,
    body: Field,
    /// Position in `docs`.
    pos: Field,
}

/// An immutable, in-RAM index. Rebuild it when the underlying data changes (cheap).
pub struct SearchIndex {
    reader: IndexReader,
    f: Fields,
    docs: Vec<Document>,
}

impl std::fmt::Debug for SearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchIndex")
            .field("len", &self.docs.len())
            .finish_non_exhaustive()
    }
}

const BOOST_TITLE: f32 = 4.0;
const BOOST_TAGS: f32 = 1.8;
const BOOST_BODY: f32 = 1.0;

impl SearchIndex {
    pub fn build(docs: Vec<Document>) -> Result<Self, SearchError> {
        let mut sb = Schema::builder();
        let f = Fields {
            kind: sb.add_text_field("kind", STRING),
            title: sb.add_text_field("title", TEXT),
            tags: sb.add_text_field("tags", TEXT),
            body: sb.add_text_field("body", TEXT),
            pos: sb.add_u64_field("pos", STORED | INDEXED),
        };
        let index = Index::create_in_ram(sb.build());
        let mut w = index.writer_with_num_threads(1, 15_000_000)?;
        for (i, d) in docs.iter().enumerate() {
            w.add_document(doc!(
                f.kind => d.kind.as_str(),
                f.title => d.title.as_str(),
                f.tags => format!("{} {}", d.tags, d.subtitle),
                f.body => d.body.as_str(),
                f.pos => i as u64,
            ))?;
        }
        w.commit()?;
        let reader = index.reader()?;
        Ok(Self { reader, f, docs })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Search, optionally restricted to some kinds. Returns at most `limit` hits.
    pub fn search(&self, q: &str, kinds: &[&str], limit: usize) -> Result<Vec<Hit>, SearchError> {
        let tokens = tokenize(q);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let last = tokens.len() - 1;
        let mut must: Vec<(Occur, Box<dyn Query>)> = tokens
            .iter()
            .enumerate()
            .map(|(i, t)| (Occur::Must, self.token_query(t, i == last)))
            .collect();
        if !kinds.is_empty() {
            let any: Vec<(Occur, Box<dyn Query>)> = kinds
                .iter()
                .map(|k| {
                    let q: Box<dyn Query> = Box::new(TermQuery::new(
                        Term::from_field_text(self.f.kind, k),
                        IndexRecordOption::Basic,
                    ));
                    (Occur::Should, q)
                })
                .collect();
            must.push((Occur::Must, Box::new(BooleanQuery::new(any))));
        }
        let query = BooleanQuery::new(must);
        let searcher = self.reader.searcher();
        let top = searcher.search(
            &query,
            &TopDocs::with_limit(limit.saturating_mul(3).max(30)).order_by_score(),
        )?;
        let mut hits: Vec<Hit> = top
            .into_iter()
            .filter_map(|(score, addr)| {
                let d: TantivyDocument = searcher.doc(addr).ok()?;
                let pos = usize::try_from(d.get_first(self.f.pos)?.as_u64()?).ok()?;
                let doc = self.docs.get(pos)?.clone();
                Some(Hit {
                    score: rerank(score, &doc, q),
                    doc,
                })
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        Ok(hits)
    }

    fn token_query(&self, token: &str, is_last: bool) -> Box<dyn Query> {
        let mut should: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for (field, boost) in [
            (self.f.title, BOOST_TITLE),
            (self.f.tags, BOOST_TAGS),
            (self.f.body, BOOST_BODY),
        ] {
            let term = Term::from_field_text(field, token);
            should.push((
                Occur::Should,
                Box::new(BoostQuery::new(
                    Box::new(TermQuery::new(term.clone(), IndexRecordOption::WithFreqs)),
                    boost,
                )),
            ));
            if is_last {
                should.push((
                    Occur::Should,
                    Box::new(BoostQuery::new(
                        Box::new(FuzzyTermQuery::new_prefix(term.clone(), 0, true)),
                        boost * 0.7,
                    )),
                ));
            }
            if token.chars().count() >= 4 {
                should.push((
                    Occur::Should,
                    Box::new(BoostQuery::new(
                        Box::new(FuzzyTermQuery::new(term, 1, true)),
                        boost * 0.4,
                    )),
                ));
            }
        }
        Box::new(BooleanQuery::new(should))
    }
}

/// Lowercase alphanumeric tokens (matches Tantivy's default tokenizer closely enough for queries).
fn tokenize(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(8)
        .map(str::to_owned)
        .collect()
}

/// Exact title matches first, then a gentle popularity boost; local things over directory entries.
#[allow(clippy::cast_precision_loss)]
fn rerank(score: f32, d: &Document, q: &str) -> f32 {
    let title = d.title.to_lowercase();
    let ql = q.trim().to_lowercase();
    let exact = if title == ql {
        3.0
    } else if title.starts_with(&ql) {
        1.6
    } else {
        1.0
    };
    let pop = 1.0 + ((d.popularity as f32) + 1.0).log10() * 0.06;
    let local = if d.kind == "discover" { 0.85 } else { 1.0 };
    score * exact * pop * local
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(kind: &str, title: &str, tags: &str, body: &str, pop: u64) -> Document {
        Document {
            kind: kind.into(),
            id: title.to_lowercase(),
            title: title.into(),
            subtitle: String::new(),
            tags: tags.into(),
            body: body.into(),
            icon: None,
            popularity: pop,
        }
    }

    fn index() -> SearchIndex {
        SearchIndex::build(vec![
            d(
                "app",
                "Immich",
                "media photos",
                "Google Photos replacement with mobile backup",
                118_000,
            ),
            d(
                "discover",
                "PhotoPrism",
                "Photo Galleries",
                "AI-powered photo app",
                36_000,
            ),
            d(
                "app",
                "Vaultwarden",
                "security passwords",
                "Bitwarden compatible password manager",
                70_000,
            ),
            d(
                "container",
                "verkos-apisix-1",
                "apisix",
                "API gateway container",
                0,
            ),
            d(
                "setting",
                "Tailscale",
                "network vpn",
                "Connect, rename, login",
                0,
            ),
        ])
        .expect("index")
    }

    fn titles(h: &[Hit]) -> Vec<&str> {
        h.iter().map(|h| h.doc.title.as_str()).collect()
    }

    #[test]
    fn exact_and_prefix() {
        let ix = index();
        assert_eq!(
            titles(&ix.search("immich", &[], 5).expect("ok"))[0],
            "Immich"
        );
        assert_eq!(
            titles(&ix.search("vaul", &[], 5).expect("ok"))[0],
            "Vaultwarden",
            "search-as-you-type"
        );
    }

    #[test]
    fn typo_tolerance() {
        assert_eq!(
            titles(&index().search("vaultwardn", &[], 5).expect("ok"))[0],
            "Vaultwarden"
        );
    }

    #[test]
    fn matches_tags_and_body_with_title_ranked_first() {
        let h = index().search("photo", &[], 5).expect("ok");
        assert!(
            titles(&h).contains(&"Immich") && titles(&h).contains(&"PhotoPrism"),
            "{:?}",
            titles(&h)
        );
        assert_eq!(titles(&h)[0], "PhotoPrism", "title match beats body match");
    }

    #[test]
    fn all_tokens_must_match_and_kind_filter() {
        let ix = index();
        assert!(ix.search("password photos", &[], 5).expect("ok").is_empty());
        let h = ix.search("photo", &["app"], 5).expect("ok");
        assert_eq!(titles(&h), ["Immich"]);
        assert!(ix.search("   ", &[], 5).expect("ok").is_empty());
    }
}
