//! Pluggable torrent search backends.
//!
//! Every site implements [`Source`] and normalizes its listings into
//! [`SearchResult`], so the CLI and TUI can render and download results without
//! knowing which site they came from.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::pirateclient::{ApiError, PirateClient};

pub mod tpb;

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
