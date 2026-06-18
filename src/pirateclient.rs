use std::time::Duration;

use reqwest::{Client, StatusCode};
use urlencoding::encode;

use crate::{
    constants::{
        BASE_URL, CATEGORY_APPLICATIONS, CATEGORY_AUDIO, CATEGORY_GAMES, CATEGORY_OTHER,
        CATEGORY_PORN, CATEGORY_VIDEO,
    },
    types::{Torrent, TorrentInfo},
};

#[derive(Debug)]
pub enum ApiError {
    Reqwest(reqwest::Error),
    RateLimited,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Reqwest(e) => write!(f, "{e}"),
            ApiError::RateLimited => f.write_str(
                "Rate limited (429): too many requests, try again later",
            ),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::Reqwest(e) => Some(e),
            ApiError::RateLimited => None,
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Reqwest(e)
    }
}

pub struct PirateClient {
    client: Client,
}

impl PirateClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("piratebay-cli")
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Torrent>, ApiError> {
        let url = format!("{BASE_URL}/q.php?q={query}");
        let res = self.client.get(&url).send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(ApiError::RateLimited);
        }
        Ok(res.json().await?)
    }

    async fn list_category(&self, category: &str) -> Result<Vec<Torrent>, ApiError> {
        let url = format!("{BASE_URL}{category}");
        let res = self.client.get(&url).send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(ApiError::RateLimited);
        }
        Ok(res.json().await?)
    }

    pub async fn list_audio(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_AUDIO).await
    }
    pub async fn list_video(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_VIDEO).await
    }
    pub async fn list_applications(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_APPLICATIONS).await
    }
    pub async fn list_games(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_GAMES).await
    }
    pub async fn list_porn(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_PORN).await
    }
    pub async fn list_other(&self) -> Result<Vec<Torrent>, ApiError> {
        self.list_category(CATEGORY_OTHER).await
    }

    const TRACKERS: &'static [&'static str] = &[
        "tracker.opentrackr.org:1337/announce",
        "open.demonoid.ch:6969/announce",
        "open.demonii.com:1337/announce",
        "open.stealth.si:80/announce",
        "tracker.torrent.eu.org:451/announce",
        "wepzone.net:6969/announce",
        "tracker2.dler.org:80/announce",
        "tracker1.myporn.club:9337/announce",
        "tracker.srv00.com:6969/announce",
        "tracker.qu.ax:6969/announce",
        "tracker.dler.org:6969/announce",
        "tracker.bittor.pw:1337/announce",
        "tracker.0x7c0.com:6969/announce",
        "tracker-udp.gbitt.info:80/announce",
        "t.overflow.biz:6969/announce",
        "run.publictracker.xyz:6969/announce",
        "retracker01-msk-virt.corbina.net:80/announce",
        "p4p.arenabg.com:1337/announce",
        "opentracker.io:6969/announce",
        "open.dstud.io:6969/announce",
    ];

    pub async fn get_info(&self, id: &str) -> Result<TorrentInfo, ApiError> {
        let trackers: Vec<String> = Self::TRACKERS
            .iter()
            .map(|t| encode(&format!("udp://{t}")).to_string())
            .collect::<Vec<_>>();
        let url = format!("{BASE_URL}/t.php?id={id}");
        let res = self.client.get(&url).send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(ApiError::RateLimited);
        }
        let mut res: TorrentInfo = res.json().await?;
        let name = encode(&res.name);
        let info_hash = &res.info_hash;
        let trackers = trackers.join("&tr=");
        res.magnet = Some(format!(
            "magnet:?xt=urn:btih:{info_hash}&dn={name}&tr={trackers}",
        ));
        Ok(res)
    }
}

impl Default for PirateClient {
    fn default() -> Self {
        Self::new()
    }
}
