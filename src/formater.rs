use chrono::{DateTime, Utc};
use colored_json::ToColoredJson;
use human_repr::HumanCount;
use owo_colors::OwoColorize;
use tabled::builder::Builder;
use tabled::Style;

use crate::sources::SearchResult;
use crate::types::TorrentInfo;

pub fn format_results(results: Vec<SearchResult>, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(&results)
                .unwrap()
                .to_colored_json_auto()
                .unwrap()
        );
        return;
    }
    let mut builder = Builder::default();
    builder.set_columns([
        "source", "added", "id", "leechers", "name", "seeders", "size", "status", "username",
    ]);
    for r in results {
        builder.add_record([
            r.source,
            r.added.map(format_date_i64).unwrap_or_default(),
            r.id,
            r.leechers.to_string(),
            r.name,
            r.seeders.to_string(),
            r.size.human_count_bytes().to_string(),
            r.status.unwrap_or_default(),
            r.uploader.unwrap_or_default(),
        ]);
    }
    println!("\n{}", builder.build().with(Style::psql()));
}

pub fn format_result(result: TorrentInfo, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(&result)
                .unwrap()
                .to_colored_json_auto()
                .unwrap()
        );
        return;
    }

    let mut builder = Builder::default();
    builder.add_record(vec!["info_hash:".yellow().to_string(), result.info_hash]);
    builder.add_record(vec!["name:".yellow().to_string(), result.name]);
    builder.add_record(vec![
        "size:".yellow().to_string(),
        result.size.human_count_bytes().to_string(),
    ]);
    builder.add_record(vec![
        "seeders:".yellow().to_string(),
        result.seeders.to_string(),
    ]);
    builder.add_record(vec![
        "leechers:".yellow().to_string(),
        result.leechers.to_string(),
    ]);
    builder.add_record(vec![
        "added_at:".yellow().to_string(),
        format_date_i64(result.added),
    ]);

    println!("\n{}", builder.build().with(Style::blank()));

    println!(
        "\n {} {}",
        "magnet_link:".yellow(),
        result.magnet.unwrap().green()
    );

    if let Some(descr) = result.descr {
        println!("\n{descr}");
    }
}

fn format_date_i64(timestamp: i64) -> String {
    let datetime: DateTime<Utc> = DateTime::from_timestamp(timestamp, 0).unwrap();
    let date = datetime.format("%Y-%m-%d");
    format!("{date}")
}
