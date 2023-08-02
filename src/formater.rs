use chrono::{DateTime, NaiveDateTime, Utc};
use colored_json::ToColoredJson;
use human_repr::HumanCount;
use owo_colors::OwoColorize;
use tabled::builder::Builder;
use tabled::{object::Columns, Format, Style};
use tabled::{Modify, Table};

use crate::types::{Torrent, TorrentInfo};

pub fn format_results(results: Vec<Torrent>, json: bool) {
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
    println!(
        "\n{}",
        Table::new(&results)
            .with(Modify::new(Columns::single(0)).with(Format::new(format_date)))
            .with(Modify::new(Columns::single(5)).with(Format::new(format_size)))
            .with(Style::psql())
    );
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

    if result.descr.is_some() {
        println!("\n{}", result.descr.unwrap().to_string());
    }
}

fn format_date(s: &str) -> String {
    if s == "added" {
        return s.to_string();
    }
    let timestamp = s.parse::<i64>().unwrap();
    let naive = NaiveDateTime::from_timestamp(timestamp, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    let date = datetime.format("%Y-%m-%d");
    return format!("{}", date);
}

fn format_date_i64(timestamp: i64) -> String {
    let naive = NaiveDateTime::from_timestamp(timestamp, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    let date = datetime.format("%Y-%m-%d");
    return format!("{}", date);
}

fn format_size(s: &str) -> String {
    if s == "size" {
        return s.to_string();
    }
    let size = s.parse::<u64>().unwrap_or(0);
    return size.human_count_bytes().to_string();
}
