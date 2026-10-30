use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::Session;
use ratatui::crossterm::event::Event;
use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use crate::sources::{self, Aggregate, SearchResult, Source, SourceCategory};

use super::download::spawn_download;

/// Tabs in the header. `All` is the free-text search scope; the rest browse a
/// [`SourceCategory`].
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

    /// The browse scope behind this tab, or `None` for free-text search.
    pub fn source_category(&self) -> Option<SourceCategory> {
        match self {
            Category::All => None,
            Category::Audio => Some(SourceCategory::Audio),
            Category::Video => Some(SourceCategory::Video),
            Category::Applications => Some(SourceCategory::Applications),
            Category::Games => Some(SourceCategory::Games),
            Category::Porn => Some(SourceCategory::Porn),
            Category::Other => Some(SourceCategory::Other),
        }
    }

    /// Category `delta` steps away, wrapping around the tab bar.
    pub fn cycle(self, delta: isize) -> Category {
        let len = Self::ORDER.len() as isize;
        let idx = (self.index() as isize + delta).rem_euclid(len) as usize;
        Self::ORDER[idx]
    }
}

#[derive(PartialEq, Eq)]
pub enum InputMode {
    /// Navigating the results list.
    Normal,
    /// Typing in the search box.
    Editing,
}

/// A download in flight, keyed by info hash in [`App::downloads`].
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
    Results(u64, Category, Result<Aggregate, String>),
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
    /// Sources enabled for this session.
    pub sources: Vec<Arc<dyn Source>>,
    /// Index into `sources` to query alone, or `None` to query them all.
    pub source_filter: Option<usize>,
    pub session: Option<Arc<Session>>,
    pub output_dir: PathBuf,
    pub tx: UnboundedSender<Msg>,

    pub query: String,
    /// Cursor position within `query`, measured in characters (0..=char_count).
    pub cursor: usize,
    pub input_mode: InputMode,
    pub category: Category,

    pub results: Vec<SearchResult>,
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
        sources: Vec<Arc<dyn Source>>,
        session: Option<Arc<Session>>,
        output_dir: PathBuf,
        tx: UnboundedSender<Msg>,
    ) -> Self {
        Self {
            sources,
            source_filter: None,
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

    // --- sources ------------------------------------------------------------

    /// The sources the next fetch will query.
    pub fn active_sources(&self) -> Vec<Arc<dyn Source>> {
        match self.source_filter {
            Some(i) => vec![self.sources[i].clone()],
            None => self.sources.clone(),
        }
    }

    /// Label for the current source scope.
    pub fn source_scope(&self) -> &'static str {
        match self.source_filter {
            Some(i) => self.sources[i].name(),
            None if self.sources.len() == 1 => self.sources[0].name(),
            None => "All sources",
        }
    }

    /// Step through "all sources" and then each source alone, and refetch.
    pub fn cycle_source(&mut self) {
        if self.sources.len() < 2 {
            return;
        }
        self.source_filter = match self.source_filter {
            None => Some(0),
            Some(i) if i + 1 < self.sources.len() => Some(i + 1),
            Some(_) => None,
        };
        self.fetch();
        if !self.loading {
            self.status = format!("Source: {}", self.source_scope());
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

    pub fn selected(&self) -> Option<&SearchResult> {
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
        let sources = self.active_sources();
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
            let res = match category.source_category() {
                None => sources::search_all(&sources, &query).await,
                Some(c) => sources::browse_all(&sources, c).await,
            };
            let res = res.map_err(|e| e.to_string());
            let _ = tx.send(Msg::Results(gen, category, res));
        });
    }

    /// Apply a settled fetch result.
    pub fn apply_results(&mut self, gen: u64, category: Category, res: Result<Aggregate, String>) {
        if gen != self.gen {
            return; // stale response
        }
        self.loading = false;
        match res {
            Ok(agg) => {
                self.results = agg.results;
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
                for (source, _) in &agg.errors {
                    self.status.push_str(&format!(" · {source} failed"));
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
        let id = t.info_hash.clone();
        let name = t.name.clone();
        let magnet = t.magnet();

        if self.downloads.contains_key(&id) {
            self.status = format!("Already downloading: {name}");
            return;
        }
        let Some(session) = self.session.clone() else {
            self.status = "Torrent session unavailable — cannot download".into();
            return;
        };

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
        let magnet = t.magnet();
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
    use crate::pirateclient::ApiError;
    use crate::sources::{NyaaSource, TpbSource};
    use tokio::sync::mpsc;

    fn test_app() -> App {
        let (tx, _rx) = mpsc::unbounded_channel();
        App::new(
            vec![Arc::new(TpbSource::new()), Arc::new(NyaaSource::new())],
            None,
            PathBuf::from("/tmp/piratebay-test"),
            tx,
        )
    }

    fn torrent(id: &str, name: &str) -> SearchResult {
        SearchResult {
            source: "tpb".into(),
            id: id.into(),
            name: name.into(),
            info_hash: format!("{id:0>40}"),
            size: 1024,
            seeders: 10,
            leechers: 1,
            added: Some(1613347200),
            category: SourceCategory::Video,
            uploader: Some("tester".into()),
            num_files: None,
            download_count: None,
            status: Some("vip".into()),
            descr: None,
            magnet: None,
        }
    }

    fn ok(results: Vec<SearchResult>) -> Result<Aggregate, String> {
        Ok(Aggregate {
            results,
            errors: Vec::new(),
        })
    }

    #[test]
    fn category_tabs_map_to_source_categories() {
        assert_eq!(Category::All.source_category(), None);
        assert_eq!(
            Category::Games.source_category(),
            Some(SourceCategory::Games)
        );
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
        let rows = vec![torrent("1", "First"), torrent("2", "Second")];
        app.apply_results(app.gen, Category::Video, ok(rows));
        assert_eq!(app.results.len(), 2);
        assert_eq!(app.table_state.selected(), Some(0));
        assert!(app.status.contains("2 results for Video"));
        assert!(!app.loading);
    }

    #[test]
    fn apply_results_reports_empty() {
        let mut app = test_app();
        app.apply_results(app.gen, Category::All, ok(vec![]));
        assert!(app.results.is_empty());
        assert_eq!(app.table_state.selected(), None);
        assert_eq!(app.status, "No results found");
    }

    #[test]
    fn apply_results_flags_failed_sources() {
        let mut app = test_app();
        let agg = Aggregate {
            results: vec![torrent("1", "Movie")],
            errors: vec![("nyaa", ApiError::RateLimited)],
        };
        app.apply_results(app.gen, Category::Video, Ok(agg));
        assert_eq!(app.results.len(), 1);
        assert!(app.status.ends_with("· nyaa failed"));
    }

    #[test]
    fn cycle_source_walks_each_source_then_all() {
        let mut app = test_app();
        assert_eq!(app.active_sources().len(), 2);
        assert_eq!(app.source_scope(), "All sources");
        app.cycle_source();
        assert_eq!(app.source_scope(), "The Pirate Bay");
        assert_eq!(app.active_sources().len(), 1);
        app.cycle_source();
        assert_eq!(app.source_scope(), "Nyaa");
        app.cycle_source();
        assert_eq!(app.source_filter, None);
        // Nothing to fetch with an empty query, so the status names the scope.
        assert_eq!(app.status, "Source: All sources");
    }

    #[test]
    fn apply_results_ignores_stale_generation() {
        let mut app = test_app();
        app.gen = 5;
        app.apply_results(4, Category::All, ok(vec![torrent("1", "Late")]));
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
        let rows = (0..3).map(|i| torrent(&i.to_string(), "T")).collect();
        app.apply_results(app.gen, Category::Video, ok(rows));

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
        app.apply_results(app.gen, Category::Video, ok(vec![torrent("1", "Movie")]));
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
