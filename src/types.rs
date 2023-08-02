use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(Serialize, Deserialize, Debug, Tabled)]
#[serde(rename_all = "camelCase")]
pub struct Torrent {
    pub added: String,
    #[tabled(skip)]
    pub category: String,
    #[tabled(skip)]
    pub descr: Option<String>,
    #[tabled(skip)]
    pub download_count: Option<String>,
    pub id: String,
    #[serde(rename = "info_hash")]
    #[tabled(skip)]
    pub info_hash: String,
    pub leechers: String,
    pub name: String,
    #[serde(rename = "num_files")]
    #[tabled(skip)]
    pub num_files: String,
    pub seeders: String,
    pub size: String,
    pub status: String,
    pub username: String,
    #[tabled(skip)]
    pub magnet: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Tabled)]
#[serde(rename_all = "camelCase")]
pub struct TorrentInfo {
    pub added: i64,
    #[tabled(skip)]
    pub category: u32,
    #[tabled(skip)]
    pub descr: Option<String>,
    #[tabled(skip)]
    pub download_count: Option<String>,
    pub id: u32,
    #[serde(rename = "info_hash")]
    #[tabled(skip)]
    pub info_hash: String,
    pub leechers: u32,
    pub name: String,
    #[serde(rename = "num_files")]
    #[tabled(skip)]
    pub num_files: u32,
    pub seeders: u32,
    pub size: u64,
    pub status: String,
    pub username: String,
    #[tabled(skip)]
    pub magnet: Option<String>,
}
