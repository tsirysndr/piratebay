use async_trait::async_trait;
use chrono::DateTime;
use reqwest::Client;
use urlencoding::encode;

use crate::pirateclient::ApiError;

use super::rss::{self, number_after, RssItem};
use super::{base_url, check_status, http_client, SearchResult, Source, SourceCategory};

/// LimeTorrents, via its search and per-category RSS feeds. The site hops
/// between domains; override it with `PIRATEBAY_LIMETORRENTS_URL`.
pub struct LimeTorrentsSource {
    client: Client,
    base_url: String,
}

impl LimeTorrentsSource {
    pub fn new() -> Self {
        Self {
            client: http_client(),
            base_url: base_url("PIRATEBAY_LIMETORRENTS_URL", "https://www.limetorrents.fun"),
        }
    }

    async fn fetch(&self, url: &str) -> Result<Vec<SearchResult>, ApiError> {
        let res = check_status(self.client.get(url).send().await?)?;
        parse_feed(&res.text().await?)
    }
}

impl Default for LimeTorrentsSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for LimeTorrentsSource {
    fn id(&self) -> &'static str {
        "limetorrents"
    }

    fn name(&self) -> &'static str {
        "LimeTorrents"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError> {
        self.fetch(&format!("{}/searchrss/{}/", self.base_url, encode(query)))
            .await
    }

    async fn browse(&self, category: SourceCategory) -> Result<Vec<SearchResult>, ApiError> {
        let id = match category {
            SourceCategory::Audio => 17,
            SourceCategory::Video => 16,
            SourceCategory::Applications => 2,
            SourceCategory::Games => 8,
            SourceCategory::Other => 21,
            SourceCategory::Porn => return Err(ApiError::Unsupported),
        };
        self.fetch(&format!("{}/rss/{id}/", self.base_url)).await
    }
}

fn parse_feed(xml: &str) -> Result<Vec<SearchResult>, ApiError> {
    Ok(rss::parse_items(xml)?
        .iter()
        .filter_map(to_result)
        .collect())
}

/// The info hash only appears in the `.torrent` enclosure URL
/// (`…/torrent/<HASH>.torrent?title=…`); items without it are dropped.
fn to_result(item: &RssItem) -> Option<SearchResult> {
    let info_hash = item
        .get("enclosure@url")
        .split("/torrent/")
        .nth(1)?
        .split(".torrent")
        .next()
        .filter(|h| h.len() == 40)?;
    let descr = item.get("description");
    let size = match item.get("size").parse() {
        Ok(size) => size,
        Err(_) => item.get("enclosure@length").parse().unwrap_or(0),
    };
    Some(SearchResult {
        source: "limetorrents".into(),
        // link ends in "-torrent-<id>.html"
        id: item
            .get("link")
            .rsplit("-torrent-")
            .next()
            .unwrap_or_default()
            .trim_end_matches(".html")
            .to_string(),
        name: item.get("title").to_string(),
        info_hash: info_hash.to_string(),
        size,
        seeders: number_after(descr, "Seeds").unwrap_or(0),
        leechers: number_after(descr, "Leechers").unwrap_or(0),
        added: DateTime::parse_from_rfc2822(item.get("pubDate"))
            .ok()
            .map(|d| d.timestamp()),
        category: category_from_name(item.get("category")),
        uploader: None,
        num_files: None,
        download_count: None,
        status: None,
        descr: None,
        magnet: None,
    })
}

/// Category names look like `Movies`, `Games - PC` or `Other - Audio Books`.
fn category_from_name(name: &str) -> SourceCategory {
    let main = name.split(" - ").next().unwrap_or_default().trim();
    match main {
        "Movies" | "TV shows" | "Anime" | "Anime." => SourceCategory::Video,
        "Music" => SourceCategory::Audio,
        "Applications" => SourceCategory::Applications,
        "Games" => SourceCategory::Games,
        _ => SourceCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_feed() {
        let results = parse_feed(include_str!("fixtures/limetorrents_search.xml")).unwrap();
        assert_eq!(results.len(), 2);
        let r = &results[0];
        assert_eq!(r.source, "limetorrents");
        assert_eq!(r.id, "19390961");
        assert_eq!(r.name, "Clinton D., Negus C. Ubuntu Linux Bible 11ed 2025");
        assert_eq!(r.info_hash, "232CD67EB3FFBD7C37BF9EC3EE887417E5AE1EE6");
        assert_eq!(r.size, 8554738);
        assert_eq!(r.seeders, 35);
        assert_eq!(r.leechers, 0);
        assert!(r.added.is_some());
        assert_eq!(r.category, SourceCategory::Other);
    }

    #[test]
    fn parses_category_feed() {
        let results = parse_feed(include_str!("fixtures/limetorrents_browse.xml")).unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.id, "20071992");
        assert_eq!(r.info_hash, "1681F7DD25C5B9C256B04EA60CC8C23B179FC000");
        assert_eq!(r.size, 19103071550);
        assert_eq!(r.category, SourceCategory::Video);
        assert_eq!(r.added, Some(1793124005));
    }

    #[test]
    fn maps_category_names() {
        assert_eq!(category_from_name("Movies"), SourceCategory::Video);
        assert_eq!(category_from_name("TV shows"), SourceCategory::Video);
        assert_eq!(
            category_from_name("Music - Music Videos"),
            SourceCategory::Audio
        );
        assert_eq!(category_from_name("Games - PC"), SourceCategory::Games);
        assert_eq!(
            category_from_name("Applications - Mac"),
            SourceCategory::Applications
        );
        assert_eq!(
            category_from_name("Other - Audio Books"),
            SourceCategory::Other
        );
    }
}
