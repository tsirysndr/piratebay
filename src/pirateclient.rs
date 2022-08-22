use std::time::Duration;

use surf::{Client, Config, Url};

use crate::{
    constants::{
        BASE_URL, CATEGORY_APPLICATIONS, CATEGORY_AUDIO, CATEGORY_GAMES, CATEGORY_OTHER,
        CATEGORY_PORN, CATEGORY_VIDEO,
    },
    types::Torrent,
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
            .get(format!("/api.php?url=/q.php?q={}", query))
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
}
