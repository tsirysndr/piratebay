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
    pub download_count: String,
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
