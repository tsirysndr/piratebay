use clap::{arg, Arg, ArgAction, Command};
use piratebay::{
    formater::{format_result, format_results},
    pirateclient::PirateClient,
};
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
            Command::new("info")
                .about("Get information about a torrent")
                .arg(
                    Arg::with_name("id")
                        .help("The torrent id")
                        .required(true)
                        .index(1),
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
        .arg(arg!(-j --json ... "Output results in json format").required(false))
}

#[tokio::main]
async fn main() -> Result<(), surf::Error> {
    let client = PirateClient::new();
    let matches = cli().get_matches();

    let json = matches.is_present("json");

    match matches.subcommand() {
        Some(("search", sub_matches)) => {
            let query = encode(sub_matches.get_one::<String>("query").unwrap());
            format_results(client.search(&query).await?, json);
        }
        Some(("info", sub_matches)) => {
            let id = sub_matches.get_one::<String>("id").unwrap();
            format_result(client.get_info(&id).await?, json);
        }
        Some(("category", sub_matches)) => {
            if *sub_matches.get_one::<bool>("audio").unwrap() {
                format_results(client.list_audio().await?, json);
            }
            if *sub_matches.get_one::<bool>("video").unwrap() {
                format_results(client.list_video().await?, json);
            }
            if *sub_matches.get_one::<bool>("applications").unwrap() {
                format_results(client.list_applications().await?, json);
            }
            if *sub_matches.get_one::<bool>("games").unwrap() {
                format_results(client.list_games().await?, json);
            }
            if *sub_matches.get_one::<bool>("porn").unwrap() {
                format_results(client.list_porn().await?, json);
            }
            if *sub_matches.get_one::<bool>("other").unwrap() {
                format_results(client.list_other().await?, json);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
