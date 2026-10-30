use async_trait::async_trait;
use chrono::DateTime;
use reqwest::Client;
use urlencoding::encode;

use crate::pirateclient::{ApiError, PirateClient};

use super::rss::{self, RssItem};
use super::{
    base_url, check_status, http_client, parse_size, SearchResult, Source, SourceCategory,
};

const TRACKER: &str = "http://nyaa.tracker.wf:7777/announce";

/// Nyaa (anime, music, software…), via its RSS feed — which carries the info
/// hash, size and peer counts, so no HTML scraping is needed.
pub struct NyaaSource {
    client: Client,
    base_url: String,
}

impl NyaaSource {
    pub fn new() -> Self {
        Self {
            client: http_client(),
            base_url: base_url("PIRATEBAY_NYAA_URL", "https://nyaa.si"),
        }
    }

    async fn fetch(&self, url: &str) -> Result<Vec<SearchResult>, ApiError> {
        let res = check_status(self.client.get(url).send().await?)?;
        parse_feed(&res.text().await?)
    }
}

impl Default for NyaaSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for NyaaSource {
    fn id(&self) -> &'static str {
        "nyaa"
    }

    fn name(&self) -> &'static str {
        "Nyaa"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError> {
        let url = format!("{}/?page=rss&c=0_0&f=0&q={}", self.base_url, encode(query));
        self.fetch(&url).await
    }

    async fn browse(&self, category: SourceCategory) -> Result<Vec<SearchResult>, ApiError> {
        let code = match category {
            SourceCategory::Audio => "2_0",
            SourceCategory::Video => "1_0",
            SourceCategory::Applications => "6_1",
            SourceCategory::Games => "6_2",
            SourceCategory::Other => "3_0",
            // Adult content lives on a separate site (sukebei).
            SourceCategory::Porn => return Err(ApiError::Unsupported),
        };
        self.fetch(&format!("{}/?page=rss&c={code}&f=0", self.base_url))
            .await
    }
}

fn parse_feed(xml: &str) -> Result<Vec<SearchResult>, ApiError> {
    Ok(rss::parse_items(xml)?
        .iter()
        .filter_map(to_result)
        .collect())
}

/// Items without an info hash can't be downloaded, so they are dropped.
fn to_result(item: &RssItem) -> Option<SearchResult> {
    let info_hash = item.get("nyaa:infoHash");
    if info_hash.is_empty() {
        return None;
    }
    let name = item.get("title");
    let status = if item.get("nyaa:trusted") == "Yes" {
        Some("trusted".to_string())
    } else if item.get("nyaa:remake") == "Yes" {
        Some("remake".to_string())
    } else {
        None
    };
    let magnet = format!(
        "{}&tr={}",
        PirateClient::magnet_for(info_hash, name),
        encode(TRACKER)
    );
    Some(SearchResult {
        source: "nyaa".into(),
        // guid is the view URL, e.g. https://nyaa.si/view/2179440
        id: item
            .get("guid")
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string(),
        name: name.to_string(),
        info_hash: info_hash.to_string(),
        size: parse_size(item.get("nyaa:size")).unwrap_or(0),
        seeders: item.get("nyaa:seeders").parse().unwrap_or(0),
        leechers: item.get("nyaa:leechers").parse().unwrap_or(0),
        added: DateTime::parse_from_rfc2822(item.get("pubDate"))
            .ok()
            .map(|d| d.timestamp()),
        category: category_from_id(item.get("nyaa:categoryId")),
        uploader: None,
        num_files: None,
        download_count: item.get("nyaa:downloads").parse().ok(),
        status,
        descr: None,
        magnet: Some(magnet),
    })
}

/// Nyaa category ids are `<main>_<sub>`: 1 anime, 2 audio, 3 literature,
/// 4 live action, 5 pictures, 6 software (6_1 apps, 6_2 games).
fn category_from_id(id: &str) -> SourceCategory {
    match id {
        "6_2" => SourceCategory::Games,
        _ if id.starts_with("6_") => SourceCategory::Applications,
        _ if id.starts_with("1_") || id.starts_with("4_") => SourceCategory::Video,
        _ if id.starts_with("2_") => SourceCategory::Audio,
        _ => SourceCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("fixtures/nyaa.xml");

    #[test]
    fn parses_rss_items() {
        let results = parse_feed(FIXTURE).unwrap();
        assert_eq!(results.len(), 2);
        let r = &results[0];
        assert_eq!(r.source, "nyaa");
        assert_eq!(r.id, "2179440");
        assert!(r.name.starts_with("[ToonsHub] One Piece EP1157"));
        assert_eq!(r.info_hash, "65d17a4e4bdd09fd79cc0fcd0db69c5a1427339c");
        assert_eq!(r.seeders, 300);
        assert_eq!(r.leechers, 24);
        assert_eq!(r.download_count, Some(791));
        assert_eq!(r.category, SourceCategory::Video);
        assert_eq!(r.size, parse_size("1.4 GiB").unwrap());
        assert_eq!(r.added, Some(1793119189));
        assert!(r.magnet().contains("nyaa.tracker.wf"));
    }

    #[test]
    fn empty_feed_has_no_results() {
        let xml = r#"<rss version="2.0"><channel><title>Nyaa</title></channel></rss>"#;
        assert!(parse_feed(xml).unwrap().is_empty());
    }

    #[test]
    fn maps_category_ids() {
        assert_eq!(category_from_id("1_2"), SourceCategory::Video);
        assert_eq!(category_from_id("4_1"), SourceCategory::Video);
        assert_eq!(category_from_id("2_1"), SourceCategory::Audio);
        assert_eq!(category_from_id("6_1"), SourceCategory::Applications);
        assert_eq!(category_from_id("6_2"), SourceCategory::Games);
        assert_eq!(category_from_id("3_1"), SourceCategory::Other);
    }
}
