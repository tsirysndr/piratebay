//! Pluggable torrent search backends.
//!
//! Every site implements [`Source`] and normalizes its listings into
//! [`SearchResult`], so the CLI and TUI can render and download results without
//! knowing which site they came from.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::join_all;
use serde::{Deserialize, Serialize};

use reqwest::{Client, Response, StatusCode};

use crate::pirateclient::{ApiError, PirateClient};

pub mod bitsearch;
pub mod limetorrents;
pub mod nyaa;
mod rss;
pub mod torrentdownload;
pub mod tpb;

pub use bitsearch::BitsearchSource;
pub use limetorrents::LimeTorrentsSource;
pub use nyaa::NyaaSource;
pub use torrentdownload::TorrentDownloadSource;
pub use tpb::TpbSource;

/// Browse scopes shared by every source. Each source maps these onto its own
/// category scheme.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceCategory {
    Audio,
    Video,
    Applications,
    Games,
    Porn,
    Other,
}

impl SourceCategory {
    /// Short icon + label for display.
    pub fn label(&self) -> (&'static str, &'static str) {
        match self {
            SourceCategory::Audio => ("♪", "Audio"),
            SourceCategory::Video => ("▶", "Video"),
            SourceCategory::Applications => ("⚙", "Apps"),
            SourceCategory::Games => ("🎮", "Games"),
            SourceCategory::Porn => ("♥", "XXX"),
            SourceCategory::Other => ("◈", "Other"),
        }
    }
}

/// A torrent listing normalized across sources.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// Id of the [`Source`] that produced this result (e.g. `"tpb"`).
    pub source: String,
    /// Source-specific id; only unique within `source`.
    pub id: String,
    pub name: String,
    pub info_hash: String,
    /// Total size in bytes.
    pub size: u64,
    pub seeders: u32,
    pub leechers: u32,
    /// Upload time as a unix timestamp, when the source exposes it.
    pub added: Option<i64>,
    pub category: SourceCategory,
    pub uploader: Option<String>,
    pub num_files: Option<u32>,
    pub download_count: Option<u32>,
    /// Uploader trust level or similar badge (e.g. TPB's `vip`).
    pub status: Option<String>,
    pub descr: Option<String>,
    /// Magnet link provided by the source, if any. Prefer [`SearchResult::magnet`].
    pub magnet: Option<String>,
}

impl SearchResult {
    /// The source's own magnet link, or one built from the info hash with the
    /// default tracker list.
    pub fn magnet(&self) -> String {
        self.magnet
            .clone()
            .unwrap_or_else(|| PirateClient::magnet_for(&self.info_hash, &self.name))
    }
}

#[async_trait]
pub trait Source: Send + Sync {
    /// Stable machine id, used in output and on the command line.
    fn id(&self) -> &'static str;
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Free-text search. `query` is raw, sources handle their own encoding.
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError>;
    /// List the latest / top torrents in a category.
    async fn browse(&self, category: SourceCategory) -> Result<Vec<SearchResult>, ApiError>;
}

/// Every available source, in display order.
pub fn all() -> Vec<Arc<dyn Source>> {
    vec![
        Arc::new(TpbSource::new()),
        Arc::new(NyaaSource::new()),
        Arc::new(BitsearchSource::new()),
        Arc::new(LimeTorrentsSource::new()),
        Arc::new(TorrentDownloadSource::new()),
    ]
}

// --- helpers shared by source implementations -------------------------------

/// HTTP client used by the sources.
pub(crate) fn http_client() -> Client {
    Client::builder()
        .user_agent("piratebay-cli")
        .timeout(Duration::from_secs(60))
        .build()
        .expect("reqwest client")
}

/// Map rate limiting to [`ApiError::RateLimited`] and other HTTP errors to
/// [`ApiError::Reqwest`].
pub(crate) fn check_status(res: Response) -> Result<Response, ApiError> {
    if res.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::RateLimited);
    }
    Ok(res.error_for_status()?)
}

/// A site's base URL: `env` overrides `default`, so users can point a source
/// at a mirror when its domain moves. Trailing slashes are trimmed.
pub(crate) fn base_url(env: &str, default: &str) -> String {
    std::env::var(env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Parse human sizes such as `1.4 GiB`, `697.57 MB` or `512 Bytes` into bytes.
/// Torrent sites use `MB`/`GB` for binary multiples, so both spellings are
/// treated as powers of 1024.
pub(crate) fn parse_size(s: &str) -> Option<u64> {
    let (num, unit) = s.trim().split_once(' ')?;
    let num: f64 = num.replace(',', "").parse().ok()?;
    let exp = match unit.trim() {
        "B" | "Bytes" | "bytes" => 0,
        "KB" | "KiB" | "kB" => 1,
        "MB" | "MiB" => 2,
        "GB" | "GiB" => 3,
        "TB" | "TiB" => 4,
        _ => return None,
    };
    Some((num * 1024f64.powi(exp)) as u64)
}

/// Resolve a comma-separated list of source ids, or `all` for every source.
pub fn from_ids(spec: &str) -> Result<Vec<Arc<dyn Source>>, String> {
    let available = all();
    let mut picked: Vec<Arc<dyn Source>> = Vec::new();
    for id in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if id == "all" {
            return Ok(available);
        }
        let source = available.iter().find(|s| s.id() == id).ok_or_else(|| {
            let ids: Vec<_> = available.iter().map(|s| s.id()).collect();
            format!("unknown source '{id}' (available: all, {})", ids.join(", "))
        })?;
        if !picked.iter().any(|s| s.id() == id) {
            picked.push(source.clone());
        }
    }
    if picked.is_empty() {
        return Err("no source selected".into());
    }
    Ok(picked)
}

/// Results merged from several sources, plus the sources that failed.
#[derive(Debug)]
pub struct Aggregate {
    pub results: Vec<SearchResult>,
    pub errors: Vec<(&'static str, ApiError)>,
}

/// Search every source concurrently; results are merged by [`merge`] and
/// ranked by seeders.
pub async fn search_all(sources: &[Arc<dyn Source>], query: &str) -> Result<Aggregate, ApiError> {
    let batches = join_all(
        sources
            .iter()
            .map(|s| async move { (s.id(), s.search(query).await) }),
    );
    merge(batches.await, |a, b| b.seeders.cmp(&a.seeders))
}

/// Browse a category on every source concurrently; newest first.
pub async fn browse_all(
    sources: &[Arc<dyn Source>],
    category: SourceCategory,
) -> Result<Aggregate, ApiError> {
    let batches = join_all(
        sources
            .iter()
            .map(|s| async move { (s.id(), s.browse(category).await) }),
    );
    merge(batches.await, |a, b| b.added.cmp(&a.added))
}

/// Combine per-source batches: torrents seen on several sources (same info
/// hash) are kept once, with the highest peer counts. A single batch keeps the
/// source's own ordering; several are sorted with `order`.
///
/// Sources that don't support the request are skipped; failures are reported
/// in [`Aggregate::errors`] unless every source failed, in which case the
/// first error is returned.
fn merge(
    batches: Vec<(&'static str, Result<Vec<SearchResult>, ApiError>)>,
    order: impl Fn(&SearchResult, &SearchResult) -> std::cmp::Ordering,
) -> Result<Aggregate, ApiError> {
    let mut results: Vec<SearchResult> = Vec::new();
    let mut by_hash: HashMap<String, usize> = HashMap::new();
    let mut errors = Vec::new();
    let mut answered = 0;
    for (id, batch) in batches {
        let batch = match batch {
            Ok(batch) => batch,
            Err(ApiError::Unsupported) => continue,
            Err(e) => {
                errors.push((id, e));
                continue;
            }
        };
        answered += 1;
        for r in batch {
            match by_hash.get(&r.info_hash.to_lowercase()) {
                Some(&i) if results[i].seeders >= r.seeders => {}
                Some(&i) => results[i] = r,
                None => {
                    by_hash.insert(r.info_hash.to_lowercase(), results.len());
                    results.push(r);
                }
            }
        }
    }
    if answered == 0 {
        return Err(if errors.is_empty() {
            ApiError::Unsupported
        } else {
            errors.remove(0).1
        });
    }
    if answered > 1 {
        results.sort_by(order);
    }
    Ok(Aggregate { results, errors })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(source: &str, hash: &str, seeders: u32) -> SearchResult {
        SearchResult {
            source: source.into(),
            id: hash.into(),
            name: hash.into(),
            info_hash: hash.into(),
            size: 0,
            seeders,
            leechers: 0,
            added: None,
            category: SourceCategory::Other,
            uploader: None,
            num_files: None,
            download_count: None,
            status: None,
            descr: None,
            magnet: None,
        }
    }

    fn by_seeders(a: &SearchResult, b: &SearchResult) -> std::cmp::Ordering {
        b.seeders.cmp(&a.seeders)
    }

    #[test]
    fn merge_dedups_by_info_hash_and_ranks() {
        let agg = merge(
            vec![
                (
                    "tpb",
                    Ok(vec![result("tpb", "AAA", 5), result("tpb", "bbb", 1)]),
                ),
                (
                    "nyaa",
                    Ok(vec![result("nyaa", "aaa", 9), result("nyaa", "ccc", 3)]),
                ),
            ],
            by_seeders,
        )
        .unwrap();
        let got: Vec<_> = agg
            .results
            .iter()
            .map(|r| (r.source.as_str(), r.seeders))
            .collect();
        assert_eq!(got, vec![("nyaa", 9), ("nyaa", 3), ("tpb", 1)]);
        assert!(agg.errors.is_empty());
    }

    #[test]
    fn merge_keeps_single_source_order() {
        let agg = merge(
            vec![(
                "tpb",
                Ok(vec![result("tpb", "a", 1), result("tpb", "b", 9)]),
            )],
            by_seeders,
        )
        .unwrap();
        assert_eq!(agg.results[0].info_hash, "a");
    }

    #[test]
    fn merge_reports_partial_failures() {
        let agg = merge(
            vec![
                ("tpb", Err(ApiError::RateLimited)),
                ("nyaa", Ok(vec![result("nyaa", "a", 1)])),
            ],
            by_seeders,
        )
        .unwrap();
        assert_eq!(agg.results.len(), 1);
        assert_eq!(agg.errors.len(), 1);
        assert_eq!(agg.errors[0].0, "tpb");
    }

    #[test]
    fn merge_fails_when_every_source_fails() {
        let err = merge(
            vec![
                ("tpb", Err(ApiError::RateLimited)),
                ("nyaa", Err(ApiError::Unsupported)),
            ],
            by_seeders,
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::RateLimited));
    }

    #[test]
    fn merge_skips_unsupported_sources() {
        let agg = merge(
            vec![
                ("tpb", Ok(vec![result("tpb", "a", 1)])),
                ("nyaa", Err(ApiError::Unsupported)),
            ],
            by_seeders,
        )
        .unwrap();
        assert_eq!(agg.results.len(), 1);
        assert!(agg.errors.is_empty());
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("512 Bytes"), Some(512));
        assert_eq!(parse_size("1.5 KiB"), Some(1536));
        assert_eq!(parse_size("2 GiB"), Some(2 << 30));
        assert_eq!(parse_size("1 MB"), Some(1 << 20));
        assert_eq!(parse_size("1,024 KB"), Some(1 << 20));
        assert_eq!(parse_size("garbage"), None);
        assert_eq!(parse_size("3 parsecs"), None);
    }

    #[test]
    fn base_url_trims_trailing_slash() {
        assert_eq!(
            base_url("PIRATEBAY_TEST_UNSET_URL", "https://example.org/"),
            "https://example.org"
        );
    }

    #[test]
    fn from_ids_resolves_lists() {
        let ids = |spec| from_ids(spec).map(|v| v.iter().map(|s| s.id()).collect::<Vec<_>>());
        assert_eq!(
            ids("all").unwrap(),
            vec![
                "tpb",
                "nyaa",
                "bitsearch",
                "limetorrents",
                "torrentdownload"
            ]
        );
        assert_eq!(ids("nyaa, tpb,nyaa").unwrap(), vec!["nyaa", "tpb"]);
        assert!(ids("bogus").unwrap_err().contains("unknown source"));
        assert!(ids("").is_err());
    }
}
