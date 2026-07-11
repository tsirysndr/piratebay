use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::Session;
use ratatui::crossterm::event::Event;
use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use crate::pirateclient::PirateClient;
use crate::types::Torrent;

use super::download::spawn_download;

/// Categories mirror The Pirate Bay's top navigation. `All` is the free-text
/// search scope; the rest map onto apibay's numeric category listings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    All,
    Audio,
    Video,
    Applications,
    Games,
    Porn,
    Other,
}

impl Category {
    pub const ORDER: [Category; 7] = [
        Category::All,
        Category::Audio,
        Category::Video,
        Category::Applications,
        Category::Games,
        Category::Porn,
        Category::Other,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Audio => "Audio",
            Category::Video => "Video",
            Category::Applications => "Apps",
            Category::Games => "Games",
            Category::Porn => "XXX",
            Category::Other => "Other",
        }
    }

    pub fn index(&self) -> usize {
        Category::ORDER.iter().position(|c| c == self).unwrap_or(0)
    }

    /// Category `delta` steps away, wrapping around the tab bar.
    pub fn cycle(self, delta: isize) -> Category {
        let len = Self::ORDER.len() as isize;
        let idx = (self.index() as isize + delta).rem_euclid(len) as usize;
        Self::ORDER[idx]
    }
}

/// Map a torrent's numeric category code to a short icon + label for display.
pub fn category_label(code: &str) -> (&'static str, &'static str) {
    match code.chars().next() {
        Some('1') => ("♪", "Audio"),
        Some('2') => ("▶", "Video"),
        Some('3') => ("⚙", "Apps"),
        Some('4') => ("🎮", "Games"),
        Some('5') => ("♥", "XXX"),
        _ => ("◈", "Other"),
    }
}

#[derive(PartialEq, Eq)]
pub enum InputMode {
    /// Navigating the results list.
    Normal,
    /// Typing in the search box.
    Editing,
}

/// A download in flight, keyed by torrent id in [`App::downloads`].
pub struct Download {
    pub name: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: f64,
    pub done: bool,
    pub error: Option<String>,
}

/// Messages funneled into the single event loop from the input thread and the
/// async fetch / download tasks.
pub enum Msg {
    Term(Event),
    /// Results for request `gen` in `category`. Stale gens are dropped.
    Results(u64, Category, Result<Vec<Torrent>, String>),
    DlProgress {
        id: String,
        name: String,
        downloaded: u64,
        total: u64,
        speed: f64,
        done: bool,
    },
    DlError {
        id: String,
        err: String,
    },
}

pub struct App {
    pub client: Arc<PirateClient>,
    pub session: Option<Arc<Session>>,
    pub output_dir: PathBuf,
    pub tx: UnboundedSender<Msg>,

    pub query: String,
    /// Cursor position within `query`, measured in characters (0..=char_count).
    pub cursor: usize,
    pub input_mode: InputMode,
    pub category: Category,

    pub results: Vec<Torrent>,
    pub table_state: TableState,
    pub loading: bool,
    pub status: String,
    /// Monotonic request id; only the newest fetch is allowed to land.
    pub gen: u64,

    pub downloads: HashMap<String, Download>,
    /// Whether the torrent detail popup is open.
    pub show_detail: bool,
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        client: Arc<PirateClient>,
        session: Option<Arc<Session>>,
        output_dir: PathBuf,
        tx: UnboundedSender<Msg>,
    ) -> Self {
        Self {
            client,
            session,
            output_dir,
            tx,
            query: String::new(),
            cursor: 0,
            input_mode: InputMode::Editing,
            category: Category::All,
            results: Vec::new(),
            table_state: TableState::default(),
            loading: false,
            status: "Type a query and press Enter — or pick a category with ← / →".into(),
            gen: 0,
            downloads: HashMap::new(),
            show_detail: false,
            show_help: false,
            should_quit: false,
        }
    }

    // --- search-input editing (cursor is a character index) ----------------

    /// Byte offset in `query` corresponding to the current char cursor.
    fn cursor_byte(&self) -> usize {
        self.query
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len())
    }

    fn char_count(&self) -> usize {
        self.query.chars().count()
    }

    pub fn input_insert(&mut self, c: char) {
        let byte = self.cursor_byte();
        self.query.insert(byte, c);
        self.cursor += 1;
    }

    /// Delete the character before the cursor (Backspace).
    pub fn input_backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let byte = self.cursor_byte();
        self.query.remove(byte);
    }

    /// Delete the character under the cursor (Delete).
    pub fn input_delete(&mut self) {
        if self.cursor >= self.char_count() {
            return;
        }
        let byte = self.cursor_byte();
        self.query.remove(byte);
    }

    pub fn input_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn input_right(&mut self) {
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    pub fn input_home(&mut self) {
        self.cursor = 0;
    }

    pub fn input_end(&mut self) {
        self.cursor = self.char_count();
    }

    pub fn selected(&self) -> Option<&Torrent> {
        self.table_state
            .selected()
            .and_then(|i| self.results.get(i))
    }

    pub fn select_next(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => (i + 1).min(self.results.len() - 1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn select_prev(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.table_state.select(Some(i));
    }

    pub fn select_page(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let cur = self.table_state.selected().unwrap_or(0) as isize;
        // saturating_add so callers can pass isize::MAX/MIN to jump to an end.
        let next = cur
            .saturating_add(delta)
            .clamp(0, self.results.len() as isize - 1);
        self.table_state.select(Some(next as usize));
    }

    /// Kick off a fetch for the current category / query and bump the gen so
    /// in-flight older requests are ignored when they return.
    pub fn fetch(&mut self) {
        self.gen += 1;
        let gen = self.gen;
        let category = self.category;
        let client = self.client.clone();
        let tx = self.tx.clone();
        let query = self.query.trim().to_string();

        if category == Category::All && query.is_empty() {
            self.results.clear();
            self.loading = false;
            self.status = "Type a query and press Enter — or pick a category with ← / →".into();
            return;
        }

        self.loading = true;
        self.status = match category {
            Category::All => format!("Searching for \"{query}\"…"),
            c => format!("Browsing {}…", c.title()),
        };

        tokio::spawn(async move {
            let res = match category {
                Category::All => client.search(&urlencoding::encode(&query)).await,
                Category::Audio => client.list_audio().await,
                Category::Video => client.list_video().await,
                Category::Applications => client.list_applications().await,
                Category::Games => client.list_games().await,
                Category::Porn => client.list_porn().await,
                Category::Other => client.list_other().await,
            };
            let res = res.map_err(|e| e.to_string());
            let _ = tx.send(Msg::Results(gen, category, res));
        });
    }

    /// Apply a settled fetch result, dropping empty/sentinel rows apibay
    /// returns when there are no matches.
    pub fn apply_results(
        &mut self,
        gen: u64,
        category: Category,
        res: Result<Vec<Torrent>, String>,
    ) {
        if gen != self.gen {
            return; // stale response
        }
        self.loading = false;
        match res {
            Ok(mut torrents) => {
                // apibay signals "no results" with a single id=0 sentinel row.
                if torrents.len() == 1 && torrents[0].id == "0" {
                    torrents.clear();
                }
                self.results = torrents;
                if self.results.is_empty() {
                    self.table_state.select(None);
                    self.status = "No results found".into();
                } else {
                    self.table_state.select(Some(0));
                    let scope = match category {
                        Category::All => format!("\"{}\"", self.query.trim()),
                        c => c.title().to_string(),
                    };
                    self.status = format!("{} results for {}", self.results.len(), scope);
                }
            }
            Err(e) => {
                self.results.clear();
                self.table_state.select(None);
                self.status = format!("Error: {e}");
            }
        }
    }

    pub fn set_category(&mut self, category: Category) {
        if self.category != category {
            self.category = category;
            self.input_mode = InputMode::Normal;
            self.fetch();
        }
    }

    pub fn next_category(&mut self, delta: isize) {
        self.set_category(self.category.cycle(delta));
    }

    /// Start downloading the highlighted torrent, or update the status if it is
    /// already downloading / the session failed to start.
    pub fn download_selected(&mut self) {
        let Some(t) = self.selected() else {
            self.status = "Nothing selected to download".into();
            return;
        };
        let id = t.id.clone();
        let name = t.name.clone();
        let info_hash = t.info_hash.clone();

        if self.downloads.contains_key(&id) {
            self.status = format!("Already downloading: {name}");
            return;
        }
        let Some(session) = self.session.clone() else {
            self.status = "Torrent session unavailable — cannot download".into();
            return;
        };

        let magnet = PirateClient::magnet_for(&info_hash, &name);
        self.downloads.insert(
            id.clone(),
            Download {
                name: name.clone(),
                downloaded: 0,
                total: 0,
                speed: 0.0,
                done: false,
                error: None,
            },
        );
        self.status = format!("Downloading → {}", self.output_dir.display());
        spawn_download(session, magnet, id, name, self.tx.clone());
    }

    /// Copy the highlighted torrent's magnet link to the system clipboard.
    pub fn copy_magnet(&mut self) {
        let Some(t) = self.selected() else {
            self.status = "Nothing selected to copy".into();
            return;
        };
        let magnet = PirateClient::magnet_for(&t.info_hash, &t.name);
        self.status = match super::clipboard::copy(&magnet) {
            Ok(()) => "Magnet link copied to clipboard".into(),
            Err(e) => format!("Copy failed: {e}"),
        };
    }

    pub fn on_progress(
        &mut self,
        id: String,
        name: String,
        downloaded: u64,
        total: u64,
        speed: f64,
        done: bool,
    ) {
        let entry = self.downloads.entry(id).or_insert_with(|| Download {
            name,
            downloaded: 0,
            total: 0,
            speed: 0.0,
            done: false,
            error: None,
        });
        entry.downloaded = downloaded;
        entry.total = total;
        entry.speed = speed;
        entry.done = done;
    }

    pub fn on_download_error(&mut self, id: String, err: String) {
        if let Some(d) = self.downloads.get_mut(&id) {
            d.error = Some(err);
        }
    }

    /// Number of downloads still running (not finished, no error).
    pub fn active_downloads(&self) -> usize {
        self.downloads
            .values()
            .filter(|d| !d.done && d.error.is_none())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn test_app() -> App {
        let (tx, _rx) = mpsc::unbounded_channel();
        App::new(
            Arc::new(PirateClient::new()),
            None,
            PathBuf::from("/tmp/piratebay-test"),
            tx,
        )
    }

    fn torrent(id: &str, name: &str, category: &str) -> Torrent {
        Torrent {
            added: "1613347200".into(),
            category: category.into(),
            descr: None,
            download_count: None,
            id: id.into(),
            info_hash: "0123456789ABCDEF0123456789ABCDEF01234567".into(),
            leechers: "1".into(),
            name: name.into(),
            num_files: None,
            seeders: "10".into(),
            size: "1024".into(),
            status: "vip".into(),
            username: "tester".into(),
            magnet: None,
        }
    }

    #[test]
    fn category_labels_map_by_leading_digit() {
        assert_eq!(category_label("101"), ("♪", "Audio"));
        assert_eq!(category_label("207"), ("▶", "Video"));
        assert_eq!(category_label("399"), ("⚙", "Apps"));
        assert_eq!(category_label("400"), ("🎮", "Games"));
        assert_eq!(category_label("505"), ("♥", "XXX"));
        assert_eq!(category_label("605"), ("◈", "Other"));
        assert_eq!(category_label(""), ("◈", "Other"));
    }

    #[test]
    fn category_cycle_wraps_both_directions() {
        assert_eq!(Category::All.cycle(1), Category::Audio);
        assert_eq!(Category::All.cycle(-1), Category::Other);
        assert_eq!(Category::Other.cycle(1), Category::All);
        // Full loop returns to start.
        assert_eq!(Category::All.cycle(7), Category::All);
    }

    #[test]
    fn apply_results_populates_and_selects_first() {
        let mut app = test_app();
        let rows = vec![torrent("1", "First", "200"), torrent("2", "Second", "200")];
        app.apply_results(app.gen, Category::Video, Ok(rows));
        assert_eq!(app.results.len(), 2);
        assert_eq!(app.table_state.selected(), Some(0));
        assert!(app.status.contains("2 results for Video"));
        assert!(!app.loading);
    }

    #[test]
    fn apply_results_drops_no_results_sentinel() {
        let mut app = test_app();
        // apibay returns a single id=0 row to mean "nothing found".
        app.apply_results(
            app.gen,
            Category::All,
            Ok(vec![torrent("0", "No results", "0")]),
        );
        assert!(app.results.is_empty());
        assert_eq!(app.table_state.selected(), None);
        assert_eq!(app.status, "No results found");
    }

    #[test]
    fn apply_results_ignores_stale_generation() {
        let mut app = test_app();
        app.gen = 5;
        app.apply_results(4, Category::All, Ok(vec![torrent("1", "Late", "200")]));
        assert!(app.results.is_empty());
    }

    #[test]
    fn apply_results_surfaces_errors() {
        let mut app = test_app();
        app.apply_results(app.gen, Category::All, Err("boom".into()));
        assert!(app.results.is_empty());
        assert!(app.status.starts_with("Error:"));
    }

    #[test]
    fn selection_stays_within_bounds() {
        let mut app = test_app();
        let rows = (0..3)
            .map(|i| torrent(&i.to_string(), "T", "200"))
            .collect();
        app.apply_results(app.gen, Category::Video, Ok(rows));

        app.select_next(); // 0 -> 1
        app.select_next(); // 1 -> 2
        app.select_next(); // clamp at 2
        assert_eq!(app.table_state.selected(), Some(2));

        app.select_prev(); // 2 -> 1
        assert_eq!(app.table_state.selected(), Some(1));

        app.select_page(isize::MAX); // jump to bottom
        assert_eq!(app.table_state.selected(), Some(2));

        app.select_page(-(isize::MAX)); // jump to top
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn download_without_session_reports_status() {
        let mut app = test_app();
        app.apply_results(
            app.gen,
            Category::Video,
            Ok(vec![torrent("1", "Movie", "200")]),
        );
        app.download_selected();
        // No session was created, so nothing is enqueued.
        assert!(app.downloads.is_empty());
        assert!(app.status.contains("session unavailable"));
    }

    #[test]
    fn input_edits_at_cursor_position() {
        let mut app = test_app();
        for c in "ubuntu".chars() {
            app.input_insert(c);
        }
        assert_eq!(app.query, "ubuntu");
        assert_eq!(app.cursor, 6);

        // Move left twice and insert in the middle.
        app.input_left();
        app.input_left();
        assert_eq!(app.cursor, 4);
        app.input_insert('X');
        assert_eq!(app.query, "ubunXtu");
        assert_eq!(app.cursor, 5);

        // Backspace removes the char before the cursor (the 'X').
        app.input_backspace();
        assert_eq!(app.query, "ubuntu");
        assert_eq!(app.cursor, 4);

        // Delete removes the char under the cursor ('t').
        app.input_delete();
        assert_eq!(app.query, "ubunu");
        assert_eq!(app.cursor, 4);
    }

    #[test]
    fn input_cursor_movement_clamps_at_bounds() {
        let mut app = test_app();
        for c in "ab".chars() {
            app.input_insert(c);
        }
        app.input_home();
        assert_eq!(app.cursor, 0);
        app.input_left(); // already at start — no underflow
        assert_eq!(app.cursor, 0);
        app.input_backspace(); // nothing before cursor — no-op
        assert_eq!(app.query, "ab");

        app.input_end();
        assert_eq!(app.cursor, 2);
        app.input_right(); // already at end — stays
        assert_eq!(app.cursor, 2);
        app.input_delete(); // nothing under cursor — no-op
        assert_eq!(app.query, "ab");
    }

    #[test]
    fn input_handles_multibyte_chars() {
        let mut app = test_app();
        for c in "café".chars() {
            app.input_insert(c);
        }
        assert_eq!(app.cursor, 4);
        // Deleting 'é' (a multibyte char) must not panic or corrupt the string.
        app.input_backspace();
        assert_eq!(app.query, "caf");
    }

    #[test]
    fn download_progress_tracks_active_count() {
        let mut app = test_app();
        app.on_progress("a".into(), "A".into(), 50, 100, 1.0, false);
        app.on_progress("b".into(), "B".into(), 100, 100, 0.0, true);
        assert_eq!(app.downloads.len(), 2);
        assert_eq!(app.active_downloads(), 1);
    }
}
