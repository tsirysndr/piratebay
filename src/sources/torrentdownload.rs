use async_trait::async_trait;
use chrono::DateTime;
use reqwest::Client;
use urlencoding::encode;

use crate::pirateclient::ApiError;

use super::rss::{self, number_after, RssItem};
use super::{
    base_url, check_status, http_client, parse_size, SearchResult, Source, SourceCategory,
};

/// TorrentDownload, a search engine indexing other trackers, via its RSS search
/// feed. It has no category feeds, so browsing is unsupported.
pub struct TorrentDownloadSource {
    client: Client,
    base_url: String,
}

impl TorrentDownloadSource {
    pub fn new() -> Self {
        Self {
            client: http_client(),
            base_url: base_url(
                "PIRATEBAY_TORRENTDOWNLOAD_URL",
                "https://www.torrentdownload.info",
            ),
        }
    }
}

impl Default for TorrentDownloadSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for TorrentDownloadSource {
    fn id(&self) -> &'static str {
        "torrentdownload"
    }

    fn name(&self) -> &'static str {
        "TorrentDownload"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError> {
        let url = format!("{}/feed?q={}", self.base_url, encode(query));
        let res = check_status(self.client.get(&url).send().await?)?;
        parse_feed(&res.text().await?)
    }

    async fn browse(&self, _category: SourceCategory) -> Result<Vec<SearchResult>, ApiError> {
        Err(ApiError::Unsupported)
    }
}

fn parse_feed(xml: &str) -> Result<Vec<SearchResult>, ApiError> {
    Ok(rss::parse_items(xml)?
        .iter()
        .filter_map(to_result)
        .collect())
}

/// Stats live in the description:
/// `Size: 697.57 MB Seeds: 4341 , Peers: 2956 Hash: A1425E0D…`.
/// Items without a valid hash are dropped.
fn to_result(item: &RssItem) -> Option<SearchResult> {
    let descr = item.get("description");
    let info_hash = descr
        .split("Hash:")
        .nth(1)?
        .split_whitespace()
        .next()
        .filter(|h| h.len() == 40)?;
    let size = descr
        .split("Size:")
        .nth(1)
        .and_then(|s| s.split("Seeds:").next())
        .and_then(parse_size)
        .unwrap_or(0);
    Some(SearchResult {
        source: "torrentdownload".into(),
        // The site addresses torrents by info hash.
        id: info_hash.to_string(),
        name: item.get("title").to_string(),
        info_hash: info_hash.to_string(),
        size,
        seeders: number_after(descr, "Seeds").unwrap_or(0),
        leechers: number_after(descr, "Peers").unwrap_or(0),
        added: DateTime::parse_from_rfc2822(item.get("pubDate"))
            .ok()
            .map(|d| d.timestamp()),
        category: SourceCategory::Other,
        uploader: None,
        num_files: None,
        download_count: None,
        status: None,
        descr: None,
        magnet: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_feed() {
        let results = parse_feed(include_str!("fixtures/torrentdownload.xml")).unwrap();
        assert_eq!(results.len(), 2);
        let r = &results[0];
        assert_eq!(r.source, "torrentdownload");
        assert_eq!(r.name, "Ubuntu 10 04 LTS x64");
        assert_eq!(r.info_hash, "A1425E0D6630336CDD9FB320F3FFF1030098975A");
        assert_eq!(r.id, r.info_hash);
        assert_eq!(r.size, parse_size("697.57 MB").unwrap());
        assert_eq!(r.seeders, 4341);
        assert_eq!(r.leechers, 2956);
        assert_eq!(r.added, Some(1301597489));
    }

    #[test]
    fn empty_feed_has_no_results() {
        let xml = "<rss><channel><title>x</title></channel></rss><script>a<b</script>";
        assert!(parse_feed(xml).unwrap().is_empty());
    }
}
