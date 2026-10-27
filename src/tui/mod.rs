mod app;
mod clipboard;
mod download;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use librqbit::{Session, SessionOptions};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::sources::Source;

use app::{App, Category, InputMode, Msg};

/// Launch the interactive TUI. `output_dir` is where downloads are written.
pub async fn run(output_dir: PathBuf, sources: Vec<Arc<dyn Source>>) -> Result<()> {
    tokio::fs::create_dir_all(&output_dir).await.ok();

    // A shared torrent session powers every download started from the UI.
    // Disable DHT/session persistence: a TUI is ephemeral and this avoids
    // touching on-disk state (and the failures that come with it).
    let opts = SessionOptions {
        disable_dht_persistence: true,
        fastresume: false,
        persistence: None,
        ..Default::default()
    };
    let session: Option<Arc<Session>> = Session::new_with_opts(output_dir.clone(), opts).await.ok();

    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    spawn_input_thread(tx.clone());

    let mut app = App::new(sources, session, output_dir, tx);

    let mut terminal = ratatui::init();
    let result = async {
        terminal.draw(|f| ui::render(f, &mut app))?;
        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Term(ev) => handle_term(&mut app, ev),
                Msg::Results(gen, cat, res) => app.apply_results(gen, cat, res),
                Msg::DlProgress {
                    id,
                    name,
                    downloaded,
                    total,
                    speed,
                    done,
                } => app.on_progress(id, name, downloaded, total, speed, done),
                Msg::DlError { id, err } => app.on_download_error(id, err),
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
    result.map_err(|e| anyhow!(e))
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

    // Popups intercept keys first.
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
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }
            KeyCode::Char(c) => app.input_insert(c),
            _ => {}
        },
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }
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
