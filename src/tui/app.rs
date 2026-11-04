use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::Session;
use ratatui::crossterm::event::Event;
use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use crate::sources::{self, Aggregate, SearchResult, Source, SourceCategory};

use super::download::{spawn_download, spawn_pause, spawn_remove};

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

/// Where a download stands.
#[derive(Clone, Debug, PartialEq)]
pub enum DlState {
    /// Requested from the UI; the session is still resolving the magnet link.
    Queued,
    /// In the session, checking existing data or fetching metadata.
    Initializing,
    Downloading,
    Paused,
    Done,
    Error(String),
}

/// A download shown in the downloads panel, identified by its lowercase hex
/// info hash.
#[derive(Clone, Debug)]
pub struct Download {
    pub hash: String,
    pub name: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: f64,
    pub state: DlState,
}

impl Download {
    /// Still transferring (or about to).
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            DlState::Queued | DlState::Initializing | DlState::Downloading
        )
    }
}

/// Which list the navigation keys act on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Results,
    Downloads,
}

/// A pending yes/no question shown in a popup.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirm {
    /// Quit while downloads are still running.
    Quit,
    /// Remove the download with this info hash.
    Remove(String),
}

/// Messages funneled into the single event loop from the input thread and the
/// async fetch / download tasks.
pub enum Msg {
    Term(Event),
    /// Results for request `gen` in `category`. Stale gens are dropped.
    Results(u64, Category, Result<Aggregate, String>),
    /// Periodic snapshot of every torrent in the session.
    Downloads(Vec<Download>),
    /// Adding the download with this info hash failed.
    DlError {
        hash: String,
        err: String,
    },
    /// A message for the status line.
    Status(String),
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

    /// Downloads in the order they were started (or restored).
    pub downloads: Vec<Download>,
    /// Selected row in the downloads panel.
    pub dl_selected: usize,
    pub focus: Focus,
    /// Whether the session saves downloads so they resume on the next launch.
    pub persistent: bool,
    /// Queued downloads removed before reaching the session, with whether
    /// their files should be deleted; they are dropped as soon as they appear.
    pub cancelled: HashMap<String, bool>,
    pub confirm: Option<Confirm>,
    /// Whether the torrent detail popup is open.
    pub show_detail: bool,
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        sources: Vec<Arc<dyn Source>>,
        session: Option<Arc<Session>>,
        persistent: bool,
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
            downloads: Vec::new(),
            dl_selected: 0,
            focus: Focus::Results,
            persistent,
            cancelled: HashMap::new(),
            confirm: None,
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
        let hash = t.info_hash.to_lowercase();
        let name = t.name.clone();
        let magnet = t.magnet();

        if self.downloads.iter().any(|d| d.hash == hash) {
            self.status = format!("Already downloading: {name}");
            return;
        }
        let Some(session) = self.session.clone() else {
            self.status = "Torrent session unavailable — cannot download".into();
            return;
        };

        self.cancelled.remove(&hash);
        self.downloads.push(Download {
            hash: hash.clone(),
            name,
            downloaded: 0,
            total: 0,
            speed: 0.0,
            state: DlState::Queued,
        });
        self.status = format!("Downloading → {}", self.output_dir.display());
        spawn_download(session, magnet, hash, self.tx.clone());
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

    /// Merge a session snapshot: update known downloads, append new ones
    /// (e.g. restored from the previous run) and drop ones the session no
    /// longer has. Queued downloads and failed adds live only in the UI, so
    /// they are kept until they show up or are removed.
    pub fn on_downloads(&mut self, snapshot: Vec<Download>) {
        let mut seen = Vec::with_capacity(snapshot.len());
        for snap in snapshot {
            if let Some(delete_files) = self.cancelled.remove(&snap.hash) {
                // Removed while still queued: drop it now that it exists.
                if let Some(session) = self.session.clone() {
                    spawn_remove(session, snap.hash, delete_files, self.tx.clone());
                }
                continue;
            }
            seen.push(snap.hash.clone());
            match self.downloads.iter_mut().find(|d| d.hash == snap.hash) {
                Some(d) => *d = snap,
                None => self.downloads.push(snap),
            }
        }
        self.downloads.retain(|d| {
            seen.contains(&d.hash) || matches!(d.state, DlState::Queued | DlState::Error(_))
        });
        self.clamp_dl_selection();
    }

    pub fn on_download_error(&mut self, hash: String, err: String) {
        if let Some(d) = self.downloads.iter_mut().find(|d| d.hash == hash) {
            d.state = DlState::Error(err);
        }
    }

    /// Number of downloads still running.
    pub fn active_downloads(&self) -> usize {
        self.downloads.iter().filter(|d| d.is_active()).count()
    }

    /// The download for a search result's info hash, if any.
    pub fn download_for(&self, info_hash: &str) -> Option<&Download> {
        let hash = info_hash.to_lowercase();
        self.downloads.iter().find(|d| d.hash == hash)
    }

    // --- downloads panel ---------------------------------------------------

    /// Move keyboard focus between the results and the downloads panel.
    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Results if self.downloads.is_empty() => {
                self.status = "No downloads yet".into();
                Focus::Results
            }
            Focus::Results => Focus::Downloads,
            Focus::Downloads => Focus::Results,
        };
        self.clamp_dl_selection();
    }

    fn clamp_dl_selection(&mut self) {
        self.dl_selected = self.dl_selected.min(self.downloads.len().saturating_sub(1));
        if self.downloads.is_empty() {
            self.focus = Focus::Results;
        }
    }

    pub fn selected_download(&self) -> Option<&Download> {
        self.downloads.get(self.dl_selected)
    }

    pub fn dl_select_next(&mut self) {
        if self.dl_selected + 1 < self.downloads.len() {
            self.dl_selected += 1;
        }
    }

    pub fn dl_select_prev(&mut self) {
        self.dl_selected = self.dl_selected.saturating_sub(1);
    }

    /// Pause the selected download, or resume it if paused.
    pub fn toggle_pause_selected(&mut self) {
        let Some(d) = self.downloads.get_mut(self.dl_selected) else {
            return;
        };
        let pause = match d.state {
            DlState::Downloading | DlState::Initializing => true,
            DlState::Paused => false,
            DlState::Queued => {
                self.status = "Still fetching metadata — try again in a moment".into();
                return;
            }
            DlState::Done => {
                self.status = "Download already complete".into();
                return;
            }
            DlState::Error(_) => {
                self.status = "Download failed — remove it and retry".into();
                return;
            }
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        // Optimistic update; the next snapshot confirms it.
        d.state = if pause {
            DlState::Paused
        } else {
            DlState::Downloading
        };
        d.speed = 0.0;
        self.status = format!("{} {}", if pause { "Paused" } else { "Resumed" }, d.name);
        spawn_pause(session, d.hash.clone(), pause, self.tx.clone());
    }

    /// Ask whether to remove the selected download.
    pub fn request_remove_selected(&mut self) {
        if let Some(d) = self.selected_download() {
            self.confirm = Some(Confirm::Remove(d.hash.clone()));
        }
    }

    /// Remove a download from the panel and the session; `delete_files` also
    /// deletes what was downloaded so far.
    pub fn remove_download(&mut self, hash: &str, delete_files: bool) {
        let Some(pos) = self.downloads.iter().position(|d| d.hash == hash) else {
            return;
        };
        let d = self.downloads.remove(pos);
        self.clamp_dl_selection();
        self.status = format!(
            "Removed {}{}",
            d.name,
            if delete_files { " and its files" } else { "" }
        );
        if d.state == DlState::Queued {
            // Not in the session yet: drop it once the magnet resolves.
            self.cancelled.insert(d.hash, delete_files);
        } else if let Some(session) = self.session.clone() {
            spawn_remove(session, d.hash, delete_files, self.tx.clone());
        }
    }

    /// Quit, asking first if downloads are still running.
    pub fn request_quit(&mut self) {
        if self.active_downloads() > 0 && self.confirm != Some(Confirm::Quit) {
            self.confirm = Some(Confirm::Quit);
        } else {
            self.should_quit = true;
        }
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
            true,
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

    fn download(hash: &str, state: DlState) -> Download {
        Download {
            hash: hash.into(),
            name: hash.to_uppercase(),
            downloaded: 50,
            total: 100,
            speed: 1.0,
            state,
        }
    }

    #[test]
    fn download_snapshots_track_active_count() {
        let mut app = test_app();
        app.on_downloads(vec![
            download("a", DlState::Downloading),
            download("b", DlState::Done),
            download("c", DlState::Paused),
        ]);
        assert_eq!(app.downloads.len(), 3);
        assert_eq!(app.active_downloads(), 1);
    }

    #[test]
    fn snapshots_keep_queued_and_drop_vanished_downloads() {
        let mut app = test_app();
        app.downloads = vec![
            download("queued", DlState::Queued),
            download("failed", DlState::Error("boom".into())),
            download("gone", DlState::Downloading),
            download("live", DlState::Initializing),
        ];
        app.on_downloads(vec![
            download("live", DlState::Downloading),
            download("restored", DlState::Paused),
        ]);
        let hashes: Vec<_> = app.downloads.iter().map(|d| d.hash.as_str()).collect();
        assert_eq!(hashes, vec!["queued", "failed", "live", "restored"]);
        assert_eq!(app.downloads[2].state, DlState::Downloading);
    }

    #[test]
    fn download_for_matches_hash_case_insensitively() {
        let mut app = test_app();
        app.on_downloads(vec![download("abcdef", DlState::Downloading)]);
        assert!(app.download_for("ABCDEF").is_some());
        assert!(app.download_for("012345").is_none());
    }

    #[test]
    fn focus_moves_to_downloads_only_when_there_are_some() {
        let mut app = test_app();
        app.toggle_focus();
        assert_eq!(app.focus, Focus::Results);
        assert_eq!(app.status, "No downloads yet");

        app.on_downloads(vec![download("a", DlState::Downloading)]);
        app.toggle_focus();
        assert_eq!(app.focus, Focus::Downloads);
        app.toggle_focus();
        assert_eq!(app.focus, Focus::Results);
    }

    #[test]
    fn download_selection_stays_within_bounds() {
        let mut app = test_app();
        app.on_downloads(vec![
            download("a", DlState::Downloading),
            download("b", DlState::Downloading),
        ]);
        app.dl_select_prev();
        assert_eq!(app.dl_selected, 0);
        app.dl_select_next();
        app.dl_select_next();
        assert_eq!(app.dl_selected, 1);
        // The selected row vanishing pulls the selection back in range.
        app.on_downloads(vec![download("a", DlState::Downloading)]);
        assert_eq!(app.dl_selected, 0);
    }

    #[test]
    fn pause_toggle_refuses_unpausable_states() {
        let mut app = test_app();
        app.downloads = vec![download("q", DlState::Queued)];
        app.toggle_pause_selected();
        assert!(app.status.contains("metadata"));
        assert_eq!(app.downloads[0].state, DlState::Queued);

        app.downloads = vec![download("d", DlState::Done)];
        app.toggle_pause_selected();
        assert!(app.status.contains("complete"));
    }

    #[test]
    fn removing_a_queued_download_cancels_it() {
        let mut app = test_app();
        app.downloads = vec![download("q", DlState::Queued)];
        app.request_remove_selected();
        assert_eq!(app.confirm, Some(Confirm::Remove("q".into())));
        app.remove_download("q", true);
        assert!(app.downloads.is_empty());
        assert_eq!(app.cancelled.get("q"), Some(&true));

        // When it later shows up in the session it is not listed again.
        app.on_downloads(vec![download("q", DlState::Initializing)]);
        assert!(app.downloads.is_empty());
        assert!(app.cancelled.is_empty());
    }

    #[test]
    fn quitting_with_active_downloads_asks_first() {
        let mut app = test_app();
        app.request_quit();
        assert!(app.should_quit, "nothing running: quit straight away");

        let mut app = test_app();
        app.on_downloads(vec![download("a", DlState::Downloading)]);
        app.request_quit();
        assert!(!app.should_quit);
        assert_eq!(app.confirm, Some(Confirm::Quit));
        // Asking again (e.g. a second Ctrl+C) forces the quit.
        app.request_quit();
        assert!(app.should_quit);
    }
}
