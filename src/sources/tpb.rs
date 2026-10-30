use async_trait::async_trait;
use urlencoding::encode;

use crate::pirateclient::{ApiError, PirateClient};
use crate::types::Torrent;

use super::{SearchResult, Source, SourceCategory};

/// The Pirate Bay, via the apibay.org JSON API.
#[derive(Default)]
pub struct TpbSource {
    client: PirateClient,
}

impl TpbSource {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Source for TpbSource {
    fn id(&self) -> &'static str {
        "tpb"
    }

    fn name(&self) -> &'static str {
        "The Pirate Bay"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, ApiError> {
        let torrents = self.client.search(&encode(query)).await?;
        Ok(into_results(torrents))
    }

    async fn browse(&self, category: SourceCategory) -> Result<Vec<SearchResult>, ApiError> {
        let torrents = match category {
            SourceCategory::Audio => self.client.list_audio().await?,
            SourceCategory::Video => self.client.list_video().await?,
            SourceCategory::Applications => self.client.list_applications().await?,
            SourceCategory::Games => self.client.list_games().await?,
            SourceCategory::Porn => self.client.list_porn().await?,
            SourceCategory::Other => self.client.list_other().await?,
        };
        Ok(into_results(torrents))
    }
}

/// Convert apibay rows, dropping the single id=0 sentinel row it returns when
/// there are no matches.
fn into_results(torrents: Vec<Torrent>) -> Vec<SearchResult> {
    if torrents.len() == 1 && torrents[0].id == "0" {
        return Vec::new();
    }
    torrents.into_iter().map(to_result).collect()
}

fn to_result(t: Torrent) -> SearchResult {
    SearchResult {
        source: "tpb".into(),
        category: category_from_code(&t.category),
        size: t.size.parse().unwrap_or(0),
        seeders: t.seeders.parse().unwrap_or(0),
        leechers: t.leechers.parse().unwrap_or(0),
        added: t.added.parse().ok(),
        num_files: t.num_files.and_then(|n| n.parse().ok()),
        download_count: t.download_count.and_then(|n| n.parse().ok()),
        uploader: Some(t.username),
        status: Some(t.status),
        descr: t.descr,
        id: t.id,
        name: t.name,
        info_hash: t.info_hash,
        magnet: t.magnet,
    }
}

/// apibay category codes group by their leading digit (1xx audio, 2xx video…).
fn category_from_code(code: &str) -> SourceCategory {
    match code.chars().next() {
        Some('1') => SourceCategory::Audio,
        Some('2') => SourceCategory::Video,
        Some('3') => SourceCategory::Applications,
        Some('4') => SourceCategory::Games,
        Some('5') => SourceCategory::Porn,
        _ => SourceCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn torrent(id: &str, category: &str) -> Torrent {
        Torrent {
            added: "1613347200".into(),
            category: category.into(),
            descr: None,
            download_count: Some("42".into()),
            id: id.into(),
            info_hash: "0123456789ABCDEF0123456789ABCDEF01234567".into(),
            leechers: "1".into(),
            name: "Ubuntu".into(),
            num_files: Some("3".into()),
            seeders: "10".into(),
            size: "1024".into(),
            status: "vip".into(),
            username: "tester".into(),
            magnet: None,
        }
    }

    #[test]
    fn category_codes_map_by_leading_digit() {
        assert_eq!(category_from_code("101"), SourceCategory::Audio);
        assert_eq!(category_from_code("207"), SourceCategory::Video);
        assert_eq!(category_from_code("399"), SourceCategory::Applications);
        assert_eq!(category_from_code("400"), SourceCategory::Games);
        assert_eq!(category_from_code("505"), SourceCategory::Porn);
        assert_eq!(category_from_code("605"), SourceCategory::Other);
        assert_eq!(category_from_code(""), SourceCategory::Other);
    }

    #[test]
    fn converts_apibay_rows() {
        let r = into_results(vec![torrent("7", "207")]).remove(0);
        assert_eq!(r.source, "tpb");
        assert_eq!(r.id, "7");
        assert_eq!(r.size, 1024);
        assert_eq!(r.seeders, 10);
        assert_eq!(r.leechers, 1);
        assert_eq!(r.added, Some(1613347200));
        assert_eq!(r.category, SourceCategory::Video);
        assert_eq!(r.num_files, Some(3));
        assert_eq!(r.download_count, Some(42));
        assert_eq!(r.uploader.as_deref(), Some("tester"));
        assert!(r
            .magnet()
            .starts_with("magnet:?xt=urn:btih:0123456789ABCDEF"));
    }

    #[test]
    fn drops_no_results_sentinel() {
        assert!(into_results(vec![torrent("0", "0")]).is_empty());
    }
}
