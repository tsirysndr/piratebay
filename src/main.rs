use clap::{arg, Arg, ArgAction, Command};
use piratebay::{formater::format_results, pirateclient::PirateClient};
use urlencoding::encode;

fn cli() -> Command<'static> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    Command::new("piratebay")
        .version(VERSION)
        .author("Tsiry Sandratraina <tsiry.sndr@aol.com>")
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

Search for torrents on the piratebay
"#,
        )
        .subcommand_required(true)
        .subcommand(
            Command::new("search")
                .about("Search for torrents")
                .arg(
                    Arg::with_name("query")
                        .help("The search query")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("category")
                        .help("The search category")
                        .required(false)
                        .index(2),
                ),
        )
        .subcommand(
            Command::new("category")
                .about("List torrents in a category")
                .arg(
                    arg!(-a --audio ... "List torrents in audio category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-v --video ... "List torrents in video category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-p --applications ... "List torrents in applications category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-g --games ... "List torrents in games category")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-n --porn ... "List torrents in porn category").action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-o --other ... "List torrents in other category")
                        .action(ArgAction::SetTrue),
                ),
        )
}

#[tokio::main]
async fn main() -> Result<(), surf::Error> {
    let client = PirateClient::new();
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("search", sub_matches)) => {
            let query = encode(sub_matches.get_one::<String>("query").unwrap());
            format_results(client.search(&query).await?);
        }
        Some(("category", sub_matches)) => {
            if *sub_matches.get_one::<bool>("audio").unwrap() {
                format_results(client.list_audio().await?);
            }
            if *sub_matches.get_one::<bool>("video").unwrap() {
                format_results(client.list_video().await?);
            }
            if *sub_matches.get_one::<bool>("applications").unwrap() {
                format_results(client.list_applications().await?);
            }
            if *sub_matches.get_one::<bool>("games").unwrap() {
                format_results(client.list_games().await?);
            }
            if *sub_matches.get_one::<bool>("porn").unwrap() {
                format_results(client.list_porn().await?);
            }
            if *sub_matches.get_one::<bool>("other").unwrap() {
                format_results(client.list_other().await?);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

/*
EXAMPLE MAGNET LINK
magnet:?xt=urn:btih:B304E27A9DA53791FA4B5E9B6D8D258CF63F7E58&dn=Kanye%20West%20-%20Donda%202%20(2022)%20Mp3%20320kbps&tr=udp%3A%2F%2F185.193.125.139%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337&tr=udp%3A%2F%2Ftracker.openbittorrent.com%3A6969%2Fannounce&tr=udp%3A%2F%2Fmovies.zsw.ca%3A6969%2Fannounce&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce&tr=udp%3A%2F%2Ftracker.0x.tf%3A6969%2Fannounce&tr=udp%3A%2F%2Fopentracker.i2p.rocks%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.tiny-vps.com%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce&tr=udp%3A%2F%2Ftracker.internetwarriors.net%3A1337%2Fannounce&tr=udp%3A%2F%2Ftracker.dler.org%3A6969%2Fannounce
*/
