use async_trait::async_trait;
use chrono::DateTime;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use urlencoding::encode;

use crate::pirateclient::ApiError;

use super::{base_url, check_status, http_client, SearchResult, Source, SourceCategory};

/// Results requested per call (the API allows up to 100).
const LIMIT: u32 = 50;

/// Bitsearch (formerly SolidTorrents), via its JSON API. Anonymous use is
/// limited to 200 requests a day per IP; set `PIRATEBAY_BITSEARCH_API_KEY` to a
/// free account's key for 1,000.
pub struct BitsearchSource {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl BitsearchSource {
    pub fn new() -> Self {
        Self {
            client: http_client(),
            base_url: base_url("PIRATEBAY_BITSEARCH_URL", "https://bitsearch.eu"),
            api_key: std::env::var("PIRATEBAY_BITSEARCH_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty()),
        }
    }

    async fn fetch(&self, params: &str) -> Result<Vec<SearchResult>, ApiError> {
        let url = format!("{}/api/v1/search?limit={LIMIT}&{params}", self.base_url);
        let mut req = self.client.get(&url);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let res = check_status(req.send().await?)?;
        parse_response(&res.text().await?)
    }
}

impl Default for BitsearchSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for BitsearchSource {
    fn id(&self) -> &'static str {
        "bitsearch"
    }

    fn name(&self) -> &'static str {
        "Bitsearch"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError> {
        self.fetch(&format!("q={}&sort=seeders", encode(query)))
            .await
    }

    async fn browse(&self, category: SourceCategory) -> Result<Vec<SearchResult>, ApiError> {
        let id = match category {
            SourceCategory::Other => 1,
            SourceCategory::Video => 2,
            SourceCategory::Applications => 5,
            SourceCategory::Games => 6,
            SourceCategory::Audio => 7,
            SourceCategory::Porn => 10,
        };
        // `q` is mandatory; `*` matches everything.
        self.fetch(&format!("q=*&category={id}&sort=date")).await
    }
}

#[derive(Deserialize)]
struct Response {
    success: bool,
    #[serde(default)]
    results: Vec<Torrent>,
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Torrent {
    id: String,
    infohash: String,
    title: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    seeders: u32,
    #[serde(default)]
    leechers: u32,
    #[serde(default)]
    downloads: Option<u32>,
    /// A numeric id in practice, but documented as a name.
    #[serde(default)]
    category: Value,
    #[serde(default)]
    verified: bool,
    created_at: Option<String>,
}

fn parse_response(body: &str) -> Result<Vec<SearchResult>, ApiError> {
    let res: Response = serde_json::from_str(body).map_err(|e| ApiError::Parse(e.to_string()))?;
    if !res.success {
        return Err(ApiError::Parse(
            res.message.unwrap_or_else(|| "request failed".into()),
        ));
    }
    Ok(res.results.into_iter().map(to_result).collect())
}

fn to_result(t: Torrent) -> SearchResult {
    SearchResult {
        source: "bitsearch".into(),
        id: t.id,
        name: t.title,
        info_hash: t.infohash,
        size: t.size,
        seeders: t.seeders,
        leechers: t.leechers,
        added: t
            .created_at
            .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
            .map(|d| d.timestamp()),
        category: category_from(&t.category),
        uploader: None,
        num_files: None,
        download_count: t.downloads,
        status: t.verified.then(|| "verified".to_string()),
        descr: None,
        magnet: None,
    }
}

/// Category ids: 1 Other, 2 Movies, 3 TV, 4 Anime, 5 Software, 6 Games,
/// 7 Music, 8 AudioBook, 9 Ebook/Course, 10 XXX.
fn category_from(v: &Value) -> SourceCategory {
    let id = match v {
        Value::Number(n) => n.as_u64().unwrap_or(1),
        Value::String(s) => match s.as_str() {
            "Movies" | "TV" | "Anime" => 2,
            "Software" | "Softwares" => 5,
            "Games" => 6,
            "Music" => 7,
            "XXX" => 10,
            _ => 1,
        },
        _ => 1,
    };
    match id {
        2..=4 => SourceCategory::Video,
        5 => SourceCategory::Applications,
        6 => SourceCategory::Games,
        7 => SourceCategory::Audio,
        10 => SourceCategory::Porn,
        _ => SourceCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_response() {
        let results = parse_response(include_str!("fixtures/bitsearch.json")).unwrap();
        assert_eq!(results.len(), 2);
        let r = &results[0];
        assert_eq!(r.source, "bitsearch");
        assert_eq!(r.id, "68131baea48761f7a5a37bd9");
        assert_eq!(r.name, "ubuntu-24.04.2-desktop-amd64.iso");
        assert_eq!(r.info_hash, "611F70899D4E1D6A9C39CFC925F103DFEF630328");
        assert_eq!(r.size, 6343219200);
        assert_eq!(r.seeders, 165);
        assert_eq!(r.leechers, 311);
        assert_eq!(r.category, SourceCategory::Applications);
        assert_eq!(r.status, None);
    }

    #[test]
    fn reads_created_at_and_verified() {
        let body = r#"{"success":true,"results":[{"id":"x","infohash":"H","title":"T",
            "category":"Games","verified":true,"createdAt":"2024-01-15T10:30:00Z"}]}"#;
        let r = &parse_response(body).unwrap()[0];
        assert_eq!(r.added, Some(1705314600));
        assert_eq!(r.category, SourceCategory::Games);
        assert_eq!(r.status.as_deref(), Some("verified"));
    }

    #[test]
    fn surfaces_api_errors() {
        let body =
            r#"{"success":false,"error":"Missing query parameter","message":"q is required"}"#;
        let err = parse_response(body).unwrap_err();
        assert!(matches!(err, ApiError::Parse(m) if m == "q is required"));
    }

    #[test]
    fn maps_categories() {
        assert_eq!(category_from(&Value::from(3)), SourceCategory::Video);
        assert_eq!(category_from(&Value::from(10)), SourceCategory::Porn);
        assert_eq!(category_from(&Value::from(9)), SourceCategory::Other);
        assert_eq!(category_from(&Value::from("Music")), SourceCategory::Audio);
        assert_eq!(category_from(&Value::Null), SourceCategory::Other);
    }
}
