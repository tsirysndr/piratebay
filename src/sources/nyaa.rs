use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::{Client, StatusCode};
use urlencoding::encode;

use crate::pirateclient::{ApiError, PirateClient};

use super::{SearchResult, Source, SourceCategory};

const BASE_URL: &str = "https://nyaa.si";
const TRACKER: &str = "http://nyaa.tracker.wf:7777/announce";

/// Nyaa (anime, music, software…), via its RSS feed — which carries the info
/// hash, size and peer counts, so no HTML scraping is needed.
pub struct NyaaSource {
    client: Client,
}

impl NyaaSource {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("piratebay-cli")
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    async fn fetch(&self, url: &str) -> Result<Vec<SearchResult>, ApiError> {
        let res = self.client.get(url).send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(ApiError::RateLimited);
        }
        let body = res.error_for_status()?.text().await?;
        parse_rss(&body)
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
        let url = format!("{BASE_URL}/?page=rss&c=0_0&f=0&q={}", encode(query));
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
        self.fetch(&format!("{BASE_URL}/?page=rss&c={code}&f=0"))
            .await
    }
}

/// Parse a Nyaa RSS document into results.
fn parse_rss(xml: &str) -> Result<Vec<SearchResult>, ApiError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();
    let mut item: Option<Item> = None;
    let mut tag = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                tag = e.name().as_ref().to_vec();
                if tag == b"item" {
                    item = Some(Item::default());
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(item) = item.as_mut() {
                    let text = e.unescape().map_err(|e| ApiError::Parse(e.to_string()))?;
                    item.set(&tag, text.into_owned());
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"item" {
                    if let Some(r) = item.take().and_then(Item::into_result) {
                        results.push(r);
                    }
                }
                tag.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(ApiError::Parse(e.to_string())),
        }
    }
    Ok(results)
}

/// Raw text fields of one RSS `<item>`.
#[derive(Default)]
struct Item {
    title: String,
    guid: String,
    pub_date: String,
    seeders: String,
    leechers: String,
    downloads: String,
    info_hash: String,
    category_id: String,
    size: String,
    trusted: String,
    remake: String,
}

impl Item {
    fn set(&mut self, tag: &[u8], text: String) {
        let field = match tag {
            b"title" => &mut self.title,
            b"guid" => &mut self.guid,
            b"pubDate" => &mut self.pub_date,
            b"nyaa:seeders" => &mut self.seeders,
            b"nyaa:leechers" => &mut self.leechers,
            b"nyaa:downloads" => &mut self.downloads,
            b"nyaa:infoHash" => &mut self.info_hash,
            b"nyaa:categoryId" => &mut self.category_id,
            b"nyaa:size" => &mut self.size,
            b"nyaa:trusted" => &mut self.trusted,
            b"nyaa:remake" => &mut self.remake,
            _ => return,
        };
        *field = text;
    }

    /// Items without an info hash can't be downloaded, so they are dropped.
    fn into_result(self) -> Option<SearchResult> {
        if self.info_hash.is_empty() {
            return None;
        }
        let status = if self.trusted == "Yes" {
            Some("trusted".to_string())
        } else if self.remake == "Yes" {
            Some("remake".to_string())
        } else {
            None
        };
        let magnet = format!(
            "{}&tr={}",
            PirateClient::magnet_for(&self.info_hash, &self.title),
            encode(TRACKER)
        );
        Some(SearchResult {
            source: "nyaa".into(),
            // guid is the view URL, e.g. https://nyaa.si/view/2179440
            id: self.guid.rsplit('/').next().unwrap_or_default().to_string(),
            category: category_from_id(&self.category_id),
            size: parse_size(&self.size).unwrap_or(0),
            seeders: self.seeders.parse().unwrap_or(0),
            leechers: self.leechers.parse().unwrap_or(0),
            added: DateTime::parse_from_rfc2822(&self.pub_date)
                .ok()
                .map(|d| d.timestamp()),
            uploader: None,
            num_files: None,
            download_count: self.downloads.parse().ok(),
            status,
            descr: None,
            magnet: Some(magnet),
            name: self.title,
            info_hash: self.info_hash,
        })
    }
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

/// Parse Nyaa's human sizes ("1.4 GiB", "512 Bytes") into bytes.
fn parse_size(s: &str) -> Option<u64> {
    let (num, unit) = s.trim().split_once(' ')?;
    let num: f64 = num.parse().ok()?;
    let mult: u64 = match unit {
        "Bytes" | "B" => 1,
        "KiB" => 1 << 10,
        "MiB" => 1 << 20,
        "GiB" => 1 << 30,
        "TiB" => 1 << 40,
        _ => return None,
    };
    Some((num * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("fixtures/nyaa.xml");

    #[test]
    fn parses_rss_items() {
        let results = parse_rss(FIXTURE).unwrap();
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
        assert!(parse_rss(xml).unwrap().is_empty());
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("512 Bytes"), Some(512));
        assert_eq!(parse_size("1.5 KiB"), Some(1536));
        assert_eq!(parse_size("2 GiB"), Some(2 << 30));
        assert_eq!(parse_size("garbage"), None);
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
