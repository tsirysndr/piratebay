use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Torrent {
    pub added: String,
    pub category: String,
    pub descr: Option<String>,
    pub download_count: String,
    pub id: String,
    #[serde(rename = "info_hash")]
    pub info_hash: String,
    pub leechers: String,
    pub name: String,
    #[serde(rename = "num_files")]
    pub num_files: String,
    pub seeders: String,
    pub size: String,
    pub status: String,
    pub username: String,
}
