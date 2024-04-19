use std::time::Duration;

use surf::{Client, Config, Url};
use urlencoding::encode;

use crate::{
    constants::{
        BASE_URL, CATEGORY_APPLICATIONS, CATEGORY_AUDIO, CATEGORY_GAMES, CATEGORY_OTHER,
        CATEGORY_PORN, CATEGORY_VIDEO,
    },
    types::{Torrent, TorrentInfo},
};

pub struct PirateClient {
    client: Client,
}

impl PirateClient {
    pub fn new() -> Self {
        let client: Client = Config::new()
            .set_base_url(Url::parse(BASE_URL).unwrap())
            .set_timeout(Some(Duration::from_secs(60)))
            .try_into()
            .unwrap();
        Self { client }
    }
    pub async fn search(&self, query: &str) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(format!("/q.php?q={}", query))
            .recv_json::<Vec<Torrent>>()
            .await?;
        Ok(res)
    }
    pub async fn list_audio(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_AUDIO)
            .recv_json::<Vec<Torrent>>()
            .await?;
        Ok(res)
    }
    pub async fn list_video(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_VIDEO)
            .recv_json::<Vec<Torrent>>()
            .await?;
        Ok(res)
    }
    pub async fn list_applications(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_APPLICATIONS)
            .recv_json::<Vec<Torrent>>()
            .await?;
        Ok(res)
    }
    pub async fn list_games(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_GAMES)
            .recv_json::<Vec<Torrent>>()
            .await?;

        Ok(res)
    }
    pub async fn list_porn(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_PORN)
            .recv_json::<Vec<Torrent>>()
            .await?;

        Ok(res)
    }
    pub async fn list_other(&self) -> Result<Vec<Torrent>, surf::Error> {
        let res = self
            .client
            .get(CATEGORY_OTHER)
            .recv_json::<Vec<Torrent>>()
            .await?;

        Ok(res)
    }

    const TRACKERS: &'static [&'static str] = &[
        "tracker.coppersurfer.tk:6969/announce",
        "9.rarbg.to:2920/announce",
        "tracker.opentrackr.org:1337",
        "tracker.internetwarriors.net:1337/announce",
        "tracker.leechers-paradise.org:6969/announce",
        "tracker.coppersurfer.tk:6969/announce",
        "tracker.pirateparty.gr:6969/announce",
        "tracker.cyberia.is:6969/announce",
        "tracker.dler.org:6969/announce",
        "tracker.torrent.eu.org:51/announce",
        "tracker.tiny-vps.com:6969/announce",
        "tracker.0x.tf:6969/announce",
        "open.stealth.si:80/announce",
        "movies.zsw.ca:6969/announce",
        "tracker.openbittorrent.com:6969/announce",
        "185.193.125.139:6969/announce",
        "opentracker.i2p.rocks:6969/announce",
    ];

    pub async fn get_info(&self, id: &str) -> Result<TorrentInfo, surf::Error> {
        let trackers: Vec<String> = Self::TRACKERS
            .iter()
            .map(|t| encode(&format!("udp://{t}")).to_string())
            .collect::<Vec<_>>();
        let mut res = self
            .client
            .get(format!("/t.php?id={}", id))
            .recv_json::<TorrentInfo>()
            .await?;
        let name = encode(&res.name);
        let info_hash = &res.info_hash;
        let trackers = trackers.join("&tr=");
        res.magnet = Some(format!(
            "magnet:?xt=urn:btih:{info_hash}&dn={name}&tr={trackers}",
        ));
        Ok(res)
    }
}
