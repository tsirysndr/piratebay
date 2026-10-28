use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session, TorrentStatsState};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use super::app::{DlState, Download, Msg};

/// Add `magnet` to the shared session. Progress is reported by
/// [`spawn_monitor`]; only failures to add are sent back here, keyed by `hash`.
pub fn spawn_download(
    session: Arc<Session>,
    magnet: String,
    hash: String,
    tx: UnboundedSender<Msg>,
) {
    tokio::spawn(async move {
        let opts = AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        };
        let err = match session
            .add_torrent(AddTorrent::from_url(magnet), Some(opts))
            .await
        {
            Ok(resp) => match resp.into_handle() {
                Some(_) => return,
                None => "session returned no handle".to_string(),
            },
            Err(e) => format!("add torrent failed: {e}"),
        };
        let _ = tx.send(Msg::DlError { hash, err });
    });
}

/// Every 500ms, snapshot every torrent in the session — including ones restored
/// from a previous run — and send it to the UI. Speed is derived from byte
/// deltas so we stay independent of librqbit's internal estimator types.
pub fn spawn_monitor(session: Arc<Session>, tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut last: HashMap<String, (u64, Instant)> = HashMap::new();
        loop {
            interval.tick().await;
            let now = Instant::now();
            let snapshot: Vec<Download> = session.with_torrents(|torrents| {
                torrents
                    .map(|(_, handle)| snapshot(handle, &last, now))
                    .collect()
            });
            last = snapshot
                .iter()
                .map(|d| (d.hash.clone(), (d.downloaded, now)))
                .collect();
            if tx.send(Msg::Downloads(snapshot)).is_err() {
                break; // UI is gone
            }
        }
    });
}

fn snapshot(
    handle: &Arc<ManagedTorrent>,
    last: &HashMap<String, (u64, Instant)>,
    now: Instant,
) -> Download {
    let hash = handle.info_hash().as_string();
    let stats = handle.stats();
    let speed = match last.get(&hash) {
        Some(&(bytes, at)) if now > at => {
            stats.progress_bytes.saturating_sub(bytes) as f64 / (now - at).as_secs_f64()
        }
        _ => 0.0,
    };
    let state = if let Some(err) = stats.error {
        DlState::Error(err)
    } else if stats.finished {
        DlState::Done
    } else {
        match stats.state {
            TorrentStatsState::Initializing => DlState::Initializing,
            TorrentStatsState::Paused => DlState::Paused,
            TorrentStatsState::Live => DlState::Downloading,
            TorrentStatsState::Error => DlState::Error("unknown error".into()),
        }
    };
    Download {
        name: handle.name().unwrap_or_else(|| hash.clone()),
        hash,
        downloaded: stats.progress_bytes,
        total: stats.total_bytes,
        speed,
        state,
    }
}

fn find(session: &Session, hash: &str) -> Option<Arc<ManagedTorrent>> {
    session.get(TorrentIdOrHash::Hash(Id20::from_str(hash).ok()?))
}

/// Pause (`pause == true`) or resume the torrent with info hash `hash`.
pub fn spawn_pause(session: Arc<Session>, hash: String, pause: bool, tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let Some(handle) = find(&session, &hash) else {
            return;
        };
        let res = if pause {
            session.pause(&handle).await
        } else {
            session.unpause(&handle).await
        };
        if let Err(e) = res {
            let verb = if pause { "pause" } else { "resume" };
            let _ = tx.send(Msg::Status(format!("Could not {verb}: {e:#}")));
        }
    });
}

/// Drop the torrent with info hash `hash` from the session (and from the
/// persisted state), optionally deleting its files.
pub fn spawn_remove(
    session: Arc<Session>,
    hash: String,
    delete_files: bool,
    tx: UnboundedSender<Msg>,
) {
    tokio::spawn(async move {
        let Some(id) = Id20::from_str(&hash).ok() else {
            return;
        };
        if session.get(TorrentIdOrHash::Hash(id)).is_none() {
            return; // never made it into the session
        }
        if let Err(e) = session
            .delete(TorrentIdOrHash::Hash(id), delete_files)
            .await
        {
            let _ = tx.send(Msg::Status(format!("Could not remove download: {e:#}")));
        }
    });
}
