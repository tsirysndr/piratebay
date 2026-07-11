use std::path::PathBuf;

use clap::builder::styling::{Color, Effects, RgbColor, Styles};
use clap::{Arg, ArgAction, Command};
use piratebay::{
    download::download,
    formater::{format_result, format_results},
    pirateclient::PirateClient,
    tui,
};
use urlencoding::encode;

// Shared palette: teal primary, cyan secondary, purple accent, green
// highlight, orange link, sky blue, red error.
const C_PRIMARY: Color = Color::Rgb(RgbColor(0, 232, 198));
const C_SECONDARY: Color = Color::Rgb(RgbColor(0, 198, 232));
const C_ACCENT: Color = Color::Rgb(RgbColor(130, 100, 255));
const C_HIGHLIGHT: Color = Color::Rgb(RgbColor(100, 232, 130));
const C_LINK: Color = Color::Rgb(RgbColor(255, 160, 100));
const C_SKY: Color = Color::Rgb(RgbColor(0, 210, 255));
const C_ERROR: Color = Color::Rgb(RgbColor(255, 100, 100));

fn theme_styles() -> Styles {
    Styles::styled()
        .header(C_PRIMARY.on_default() | Effects::BOLD)
        .usage(C_SKY.on_default() | Effects::BOLD)
        .literal(C_SECONDARY.on_default() | Effects::BOLD)
        .placeholder(C_ACCENT.on_default())
        .error(C_ERROR.on_default() | Effects::BOLD)
        .valid(C_HIGHLIGHT.on_default() | Effects::BOLD)
        .invalid(C_LINK.on_default() | Effects::BOLD)
}

fn cli() -> Command {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    Command::new("piratebay")
        .version(VERSION)
        .author("Tsiry Sandratraina <tsiry.sndr@rocksky.app>")
        .styles(theme_styles())
        .about(
            r#"

  _____ _           _       ____
 |  __ (_)         | |     |  _ \
 | |__) | _ __ __ _| |_ ___| |_) | __ _ _   _
 |  ___/ | '__/ _` | __/ _ \  _ < / _` | | | |
 | |   | | | | (_| | ||  __/ |_) | (_| | |_| |
 |_|   |_|_|  \__,_|\__\___|____/ \__,_|\__, |
                                         __/ |
                                        |___/

Search and download torrents from The Pirate Bay
"#,
        )
        .subcommand(
            Command::new("tui")
                .about("Launch the interactive terminal UI (default when run with no arguments)")
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .help("Directory downloads are saved to (default: current directory)")
                        .value_name("DIR")
                        .num_args(1),
                ),
        )
        .subcommand(
            Command::new("search")
                .about("Search for torrents")
                .arg(
                    Arg::new("query")
                        .help("The search query")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("category")
                        .help("The search category")
                        .required(false)
                        .index(2),
                ),
        )
        .subcommand(
            Command::new("info")
                .about("Get information about a torrent")
                .arg(
                    Arg::new("id")
                        .help("The torrent id")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            Command::new("download")
                .about("Download a torrent (pure-Rust, async, with progress bar)")
                .arg(
                    Arg::new("target")
                        .help("Piratebay id, magnet: URL, or path to a .torrent file")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .help("Output directory (default: current directory)")
                        .value_name("DIR")
                        .num_args(1),
                )
                .arg(
                    Arg::new("stream")
                        .short('s')
                        .long("stream")
                        .help("Stream the largest file to stdout as it downloads — pipe to mpv/vlc")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("category")
                .about("List torrents in a category")
                .arg(
                    Arg::new("audio")
                        .short('a')
                        .long("audio")
                        .help("List torrents in audio category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("video")
                        .short('v')
                        .long("video")
                        .help("List torrents in video category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("applications")
                        .short('p')
                        .long("applications")
                        .help("List torrents in applications category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("games")
                        .short('g')
                        .long("games")
                        .help("List torrents in games category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("porn")
                        .short('n')
                        .long("porn")
                        .help("List torrents in porn category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("other")
                        .short('o')
                        .long("other")
                        .help("List torrents in other category")
                        .action(ArgAction::SetTrue),
                ),
        )
        .arg(
            Arg::new("json")
                .short('j')
                .long("json")
                .help("Output results in json format")
                .action(ArgAction::SetTrue)
                .global(true),
        )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = PirateClient::new();
    let matches = cli().get_matches();

    let json = matches.get_flag("json");

    match matches.subcommand() {
        Some(("tui", sub_matches)) => {
            let output = sub_matches
                .get_one::<String>("output")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().expect("cwd"));
            tui::run(output).await?;
        }
        None => {
            let output = std::env::current_dir().expect("cwd");
            tui::run(output).await?;
        }
        Some(("search", sub_matches)) => {
            let json = json || sub_matches.get_flag("json");
            let query = encode(sub_matches.get_one::<String>("query").unwrap());
            format_results(client.search(&query).await?, json);
        }
        Some(("info", sub_matches)) => {
            let json = json || sub_matches.get_flag("json");
            let id = sub_matches.get_one::<String>("id").unwrap();
            format_result(client.get_info(id).await?, json);
        }
        Some(("download", sub_matches)) => {
            let target = sub_matches.get_one::<String>("target").unwrap();
            let output = sub_matches
                .get_one::<String>("output")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().expect("cwd"));
            let stream = sub_matches.get_flag("stream");
            download(target, output, stream).await?;
        }
        Some(("category", sub_matches)) => {
            let json = json || sub_matches.get_flag("json");
            if sub_matches.get_flag("audio") {
                format_results(client.list_audio().await?, json);
            }
            if sub_matches.get_flag("video") {
                format_results(client.list_video().await?, json);
            }
            if sub_matches.get_flag("applications") {
                format_results(client.list_applications().await?, json);
            }
            if sub_matches.get_flag("games") {
                format_results(client.list_games().await?, json);
            }
            if sub_matches.get_flag("porn") {
                format_results(client.list_porn().await?, json);
            }
            if sub_matches.get_flag("other") {
                format_results(client.list_other().await?, json);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
