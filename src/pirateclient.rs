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
        self.client
            .get(format!("/q.php?q={}", query))
            .recv_json()
            .await
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

impl Default for PirateClient {
    fn default() -> Self {
        Self::new()
    }
}
