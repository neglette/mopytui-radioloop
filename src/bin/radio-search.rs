//! Standalone Radio Browser CLI; shares search/probe, never requires Mopidy.
use anyhow::Result;
use mopytui::radio::{Config, probe, search};
use std::time::Duration;
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "radio-search [--json] [--probe] QUERY\nSearch by name OR tags. Outputs name, codec, bitrate, URL.\n--probe verifies decoded audio (5 seconds per station).\nThis CLI does not change Mopidy playback. Existing radio-search scripts remain usable."
        );
        return Ok(());
    }
    let json = args.iter().any(|a| a == "--json");
    let check = args.iter().any(|a| a == "--probe");
    let query = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let cfg = Config::default();
    let mut stations = search::search(&query, &cfg.servers).await?;
    if check {
        let mut live = Vec::new();
        for station in stations {
            if probe::probe(&cfg.python, station.stream_url(), Duration::from_secs(5))
                .await
                .is_ok()
            {
                live.push(station);
            }
        }
        stations = live;
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&stations)?);
    } else {
        for station in stations {
            println!(
                "{}\t{}\t{}\t{}",
                station.name,
                station.codec,
                station.bitrate,
                station.stream_url()
            );
        }
    }
    Ok(())
}
