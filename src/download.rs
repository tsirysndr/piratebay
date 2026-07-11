use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session};
use owo_colors::OwoColorize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::pirateclient::PirateClient;

const STREAM_CHUNK: usize = 64 * 1024;

pub async fn download(target: &str, output: PathBuf, stream: bool) -> Result<()> {
    let add = resolve_input(target).await?;

    tokio::fs::create_dir_all(&output)
        .await
        .with_context(|| format!("create output dir {}", output.display()))?;

    let session = Session::new(output.clone())
        .await
        .map_err(|e| anyhow!("session init failed: {e}"))?;

    let opts = AddTorrentOptions {
        overwrite: true,
        ..Default::default()
    };

    let resp = session
        .add_torrent(add, Some(opts))
        .await
        .map_err(|e| anyhow!("add_torrent failed: {e}"))?;

    let handle: Arc<ManagedTorrent> = resp
        .into_handle()
        .ok_or_else(|| anyhow!("session returned no handle"))?;

    handle
        .wait_until_initialized()
        .await
        .map_err(|e| anyhow!("metadata fetch failed: {e}"))?;

    let name = handle.name().unwrap_or_else(|| "(unknown)".into());
    eprintln!(
        "\n{} {}",
        "::".truecolor(0, 232, 198).bold(),
        name.truecolor(130, 100, 255).bold()
    );
    eprintln!(
        "{} {}",
        "->".truecolor(0, 232, 198),
        output.display().truecolor(0, 210, 255)
    );

    let total = handle.stats().total_bytes;
    let pb = make_progress_bar(total);

    let progress_handle = handle.clone();
    let progress_pb = pb.clone();
    let progress_task =
        tokio::spawn(async move { drive_progress(progress_handle, progress_pb).await });

    if stream {
        let (file_id, name, size) = pick_stream_file(&handle)?;
        eprintln!(
            "{} {} {}",
            "stream:".truecolor(0, 232, 198).bold(),
            name.truecolor(0, 210, 255),
            format!("({} bytes)", size).truecolor(200, 210, 220)
        );
        let mut reader = handle
            .clone()
            .stream(file_id)
            .map_err(|e| anyhow!("stream open failed: {e}"))?;
        let mut stdout = tokio::io::stdout();
        let mut buf = vec![0u8; STREAM_CHUNK];
        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| anyhow!("stream read: {e}"))?;
            if n == 0 {
                break;
            }
            stdout
                .write_all(&buf[..n])
                .await
                .map_err(|e| anyhow!("stdout write: {e}"))?;
        }
        stdout.flush().await.ok();
    } else {
        handle
            .wait_until_completed()
            .await
            .map_err(|e| anyhow!("download failed: {e}"))?;
    }

    progress_task.abort();
    let _ = progress_task.await;
    pb.finish_with_message("done");

    eprintln!(
        "\n{} saved to {}\n",
        "✓".truecolor(100, 232, 130).bold(),
        output.display().truecolor(130, 100, 255)
    );
    Ok(())
}

async fn resolve_input(target: &str) -> Result<AddTorrent<'static>> {
    if target.starts_with("magnet:")
        || target.starts_with("http://")
        || target.starts_with("https://")
    {
        return Ok(AddTorrent::from_url(target.to_string()));
    }
    if tokio::fs::metadata(target).await.is_ok() {
        let bytes = tokio::fs::read(target)
            .await
            .with_context(|| format!("read torrent file {target}"))?;
        return Ok(AddTorrent::from_bytes(bytes));
    }
    let client = PirateClient::new();
    let info = client
        .get_info(target)
        .await
        .map_err(|e| anyhow!("piratebay lookup for id {target} failed: {e}"))?;
    let magnet = info
        .magnet
        .ok_or_else(|| anyhow!("torrent {target} has no magnet link"))?;
    Ok(AddTorrent::from_url(magnet))
}

fn make_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total.max(1));
    // 256-colour indices approximating the theme: teal 43, sky 45, orange 215,
    // muted 250 (indicatif's template syntax doesn't take truecolor RGB).
    let tmpl = "  {spinner:.43} [{elapsed_precise:.250}] \
                [{bar:32.43/238}] \
                {bytes:>10.45}/{total_bytes:.250} \
                {bytes_per_sec:>11.215}  ETA {eta:.250}";
    pb.set_style(
        ProgressStyle::with_template(tmpl)
            .unwrap()
            .progress_chars("━╸ ")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

async fn drive_progress(handle: Arc<ManagedTorrent>, pb: ProgressBar) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        interval.tick().await;
        let stats = handle.stats();
        if stats.total_bytes > 0 && pb.length() != Some(stats.total_bytes) {
            pb.set_length(stats.total_bytes);
        }
        pb.set_position(stats.progress_bytes);
        if stats.finished {
            break;
        }
    }
}

fn pick_stream_file(handle: &Arc<ManagedTorrent>) -> Result<(usize, String, u64)> {
    handle
        .with_metadata(|m| {
            let mut best: Option<(usize, &std::path::Path, u64)> = None;
            for (idx, fi) in m.file_infos.iter().enumerate() {
                if best.map(|(_, _, l)| fi.len > l).unwrap_or(true) {
                    best = Some((idx, fi.relative_filename.as_path(), fi.len));
                }
            }
            best.map(|(i, p, l)| (i, p.display().to_string(), l))
                .ok_or_else(|| anyhow!("torrent has no files"))
        })
        .map_err(|e| anyhow!("metadata access failed: {e}"))?
}
