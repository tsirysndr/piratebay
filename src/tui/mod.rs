mod app;
mod clipboard;
mod download;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use librqbit::{Session, SessionOptions, SessionPersistenceConfig};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::sources::Source;

use app::{App, Category, Confirm, Focus, InputMode, Msg};
use download::spawn_monitor;

/// Launch the interactive TUI. `output_dir` is where downloads are written.
pub async fn run(output_dir: PathBuf, sources: Vec<Arc<dyn Source>>) -> Result<()> {
    tokio::fs::create_dir_all(&output_dir).await.ok();

    let (session, persistent, status) = start_session(&output_dir).await;

    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    spawn_input_thread(tx.clone());
    if let Some(session) = &session {
        spawn_monitor(session.clone(), tx.clone());
    }

    let mut app = App::new(sources, session.clone(), persistent, output_dir, tx);
    if let Some(status) = status {
        app.status = status;
    }
    if app
        .session
        .as_ref()
        .is_some_and(|s| s.with_torrents(|t| t.count()) > 0)
    {
        // Restored downloads: start in navigation mode so `t` works right away.
        app.input_mode = InputMode::Normal;
    }

    let mut terminal = ratatui::init();
    let result = async {
        terminal.draw(|f| ui::render(f, &mut app))?;
        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Term(ev) => handle_term(&mut app, ev),
                Msg::Results(gen, cat, res) => app.apply_results(gen, cat, res),
                Msg::Downloads(snapshot) => app.on_downloads(snapshot),
                Msg::DlError { hash, err } => app.on_download_error(hash, err),
                Msg::Status(status) => app.status = status,
            }
            if app.should_quit {
                break;
            }
            terminal.draw(|f| ui::render(f, &mut app))?;
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    ratatui::restore();
    if let Some(session) = session {
        // Pause torrents cleanly so their progress is saved for next time.
        session.stop().await;
    }
    result.map_err(|e| anyhow!(e))
}

/// Start the torrent session shared by every download started from the UI.
///
/// The session is persisted (with fastresume) under the user's data directory,
/// so unfinished downloads resume — paused ones stay paused — the next time the
/// TUI starts. If that fails, fall back to an in-memory session. Returns the
/// session, whether it is persistent, and an optional status message.
async fn start_session(
    output_dir: &std::path::Path,
) -> (Option<Arc<Session>>, bool, Option<String>) {
    let persistent = async {
        let folder = directories::ProjectDirs::from("", "", "piratebay")
            .context("cannot determine the data directory")?
            .data_dir()
            .join("session");
        Session::new_with_opts(
            output_dir.to_path_buf(),
            SessionOptions {
                disable_dht_persistence: true,
                fastresume: true,
                persistence: Some(SessionPersistenceConfig::Json {
                    folder: Some(folder),
                }),
                ..Default::default()
            },
        )
        .await
    }
    .await;

    let err = match persistent {
        Ok(session) => {
            let restored = session.with_torrents(|torrents| torrents.count());
            let status = (restored > 0).then(|| {
                format!("Restored {restored} download(s) from the previous session — press t to manage them")
            });
            return (Some(session), true, status);
        }
        Err(e) => e,
    };

    let ephemeral = Session::new_with_opts(
        output_dir.to_path_buf(),
        SessionOptions {
            disable_dht_persistence: true,
            fastresume: false,
            persistence: None,
            ..Default::default()
        },
    )
    .await
    .ok();
    let status = format!("Downloads won't be resumed after quitting: {err:#}");
    (ephemeral, false, Some(status))
}

/// Blocking-read terminal events on a dedicated thread and forward them into
/// the async event loop's channel.
fn spawn_input_thread(tx: UnboundedSender<Msg>) {
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(Msg::Term(ev)).is_err() {
                break;
            }
        }
    });
}

fn handle_term(app: &mut App, ev: Event) {
    let Event::Key(key) = ev else { return };
    if key.kind != KeyEventKind::Press {
        return;
    }

    // Ctrl+C always quits; with downloads running it asks first, and a second
    // Ctrl+C confirms.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.request_quit();
        return;
    }

    // Popups intercept keys first.
    if let Some(confirm) = app.confirm.clone() {
        match (confirm, key.code) {
            (Confirm::Quit, KeyCode::Char('y') | KeyCode::Enter) => app.should_quit = true,
            (Confirm::Remove(hash), KeyCode::Char('y') | KeyCode::Enter) => {
                app.remove_download(&hash, false)
            }
            (Confirm::Remove(hash), KeyCode::Char('f')) => app.remove_download(&hash, true),
            (_, KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc) => {}
            _ => return, // keep the popup open on any other key
        }
        app.confirm = None;
        return;
    }
    if app.show_help {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                app.show_help = false
            }
            _ => {}
        }
        return;
    }
    if app.show_detail {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => app.show_detail = false,
            KeyCode::Char('d') => app.download_selected(),
            KeyCode::Char('c') | KeyCode::Char('y') => app.copy_magnet(),
            _ => {}
        }
        return;
    }

    match app.input_mode {
        InputMode::Editing => match key.code {
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Enter => {
                app.input_mode = InputMode::Normal;
                app.category = Category::All;
                app.fetch();
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_left(),
            KeyCode::Right => app.input_right(),
            KeyCode::Home => app.input_home(),
            KeyCode::End => app.input_end(),
            KeyCode::Char(c) => app.input_insert(c),
            _ => {}
        },
        InputMode::Normal if app.focus == Focus::Downloads => match key.code {
            KeyCode::Char('q') => app.request_quit(),
            KeyCode::Down | KeyCode::Char('j') => app.dl_select_next(),
            KeyCode::Up | KeyCode::Char('k') => app.dl_select_prev(),
            KeyCode::Char('p') | KeyCode::Char(' ') => app.toggle_pause_selected(),
            KeyCode::Char('x') | KeyCode::Delete => app.request_remove_selected(),
            KeyCode::Char('t') | KeyCode::Esc => app.toggle_focus(),
            KeyCode::Char('/') | KeyCode::Char('i') => {
                app.toggle_focus();
                app.category = Category::All;
                app.input_mode = InputMode::Editing;
                app.input_end();
            }
            KeyCode::Char('?') => app.show_help = true,
            _ => {}
        },
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => app.request_quit(),
            KeyCode::Char('t') => app.toggle_focus(),
            KeyCode::Char('/') | KeyCode::Char('i') => {
                app.category = Category::All;
                app.input_mode = InputMode::Editing;
                app.input_end();
            }
            KeyCode::Down | KeyCode::Char('j') => app.select_next(),
            KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
            KeyCode::PageDown => app.select_page(10),
            KeyCode::PageUp => app.select_page(-10),
            KeyCode::Char('g') | KeyCode::Home => app.select_page(-(i32::MAX as isize)),
            KeyCode::Char('G') | KeyCode::End => app.select_page(i32::MAX as isize),
            KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => app.next_category(1),
            KeyCode::Left | KeyCode::BackTab | KeyCode::Char('h') => app.next_category(-1),
            KeyCode::Char('d') => app.download_selected(),
            KeyCode::Char('c') | KeyCode::Char('y') => app.copy_magnet(),
            KeyCode::Char('r') => app.fetch(),
            KeyCode::Char('s') => app.cycle_source(),
            KeyCode::Enter => {
                if app.selected().is_some() {
                    app.show_detail = true;
                }
            }
            KeyCode::Char('?') => app.show_help = true,
            KeyCode::Esc => {}
            _ => {}
        },
    }
}
