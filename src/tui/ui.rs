use chrono::DateTime;
use human_repr::{HumanCount, HumanThroughput};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, Wrap,
};
use ratatui::Frame;

use super::app::{App, Category, InputMode};

// Dark-canvas palette: teal primary, cyan secondary, purple accent, green
// highlight, orange link, sky blue, red error, light-gray muted. Borders keep
// the light ship-hull brown.
const PRIMARY: Color = Color::Rgb(0, 232, 198); // teal — brand accent
const SECONDARY: Color = Color::Rgb(0, 198, 232); // cyan
const ACCENT: Color = Color::Rgb(130, 100, 255); // purple
const HIGHLIGHT: Color = Color::Rgb(100, 232, 130); // green — seeders
const LINK: Color = Color::Rgb(255, 160, 100); // orange — links
const SKY: Color = Color::Rgb(0, 210, 255); // sky blue — titles / names
const ERROR: Color = Color::Rgb(255, 100, 100); // red — leechers / errors
const MUTED: Color = Color::Rgb(200, 210, 220); // muted meta text
const TEXT: Color = Color::Rgb(0xEC, 0xEC, 0xEC); // near-white body text
const BROWN: Color = Color::Rgb(0xCB, 0xA1, 0x72); // light ship-hull brown — borders
const SEL_BG: Color = Color::Rgb(0x24, 0x2C, 0x36); // selected-row background

pub fn render(f: &mut Frame, app: &mut App) {
    let has_downloads = !app.downloads.is_empty();
    let dl_height = if has_downloads {
        (app.downloads.len().min(6) as u16) + 2
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header + search
            Constraint::Length(3), // category tabs
            Constraint::Min(5),    // results
            Constraint::Length(dl_height),
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_results(f, app, chunks[2]);
    if has_downloads {
        render_downloads(f, app, chunks[3]);
    }
    render_footer(f, app, chunks[4]);

    if app.show_detail {
        render_detail(f, app);
    }
    if app.show_help {
        render_help(f);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(20)])
        .split(area);

    let logo = Paragraph::new(Line::from(vec![
        Span::styled(
            " ☠ ",
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled("THE ", Style::default().fg(TEXT).bold()),
        Span::styled("PIRATE ", Style::default().fg(ACCENT).bold()),
        Span::styled("BAY", Style::default().fg(TEXT).bold()),
        Span::styled(" ⛵", Style::default().fg(SKY)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BROWN)),
    );
    f.render_widget(logo, cols[0]);

    let editing = app.input_mode == InputMode::Editing;
    let border = if editing { ACCENT } else { MUTED };
    let title = if app.category == Category::All {
        " Search "
    } else {
        " Filter (switch to All to search) "
    };
    let query_line = if app.query.is_empty() && !editing {
        Span::styled(
            "type to search torrents…",
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        )
    } else {
        Span::styled(app.query.clone(), Style::default().fg(SKY).bold())
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(PRIMARY)),
        query_line,
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .title(Span::styled(title, Style::default().fg(border).bold())),
    );
    f.render_widget(search, cols[1]);

    if editing {
        // Place the cursor at its char index within the query (4 = border + "🔍 ").
        let x = cols[1].x + 4 + app.cursor as u16;
        let y = cols[1].y + 1;
        f.set_cursor_position((x.min(cols[1].x + cols[1].width.saturating_sub(2)), y));
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED))
        .title(Span::styled(
            " Categories ",
            Style::default().fg(PRIMARY).bold(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut spans: Vec<Span> = Vec::new();
    for (i, cat) in Category::ORDER.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(MUTED)));
        }
        let selected = *cat == app.category;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        };
        spans.push(Span::styled(format!(" {} ", cat.title()), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn render_results(f: &mut Frame, app: &mut App, area: Rect) {
    let title = if app.loading {
        Span::styled(" ⟳ loading… ", Style::default().fg(LINK).bold())
    } else {
        Span::styled(
            format!(" Results ({}) ", app.results.len()),
            Style::default().fg(SKY).bold(),
        )
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BROWN))
        .title(title);

    if app.results.is_empty() {
        let msg = if app.loading {
            "Fetching torrents…"
        } else {
            "No torrents to show.\n\nPress  /  to search, or  ← →  to browse a category."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center)
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("Category"),
        Cell::from("Name"),
        Cell::from("Size"),
        Cell::from("Uploaded"),
        Cell::from("SE"),
        Cell::from("LE"),
        Cell::from("Uploader"),
    ])
    .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    .bottom_margin(1);

    let rows = app.results.iter().map(|t| {
        let (icon, label) = t.category.label();
        let dl = app.downloads.get(&t.info_hash);
        let marker = match dl {
            Some(d) if d.error.is_some() => Span::styled("✗", Style::default().fg(ERROR)),
            Some(d) if d.done => Span::styled("✓", Style::default().fg(HIGHLIGHT)),
            Some(_) => Span::styled(
                "▼",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ),
            None => Span::raw(" "),
        };
        Row::new(vec![
            Cell::from(Line::from(marker)),
            Cell::from(format!("{icon} {label}")).style(Style::default().fg(MUTED)),
            Cell::from(t.name.clone()).style(Style::default().fg(SKY)),
            Cell::from(human_bytes(t.size)).style(Style::default().fg(SECONDARY)),
            Cell::from(human_date(t.added)).style(Style::default().fg(MUTED)),
            Cell::from(t.seeders.to_string()).style(Style::default().fg(HIGHLIGHT)),
            Cell::from(t.leechers.to_string()).style(Style::default().fg(ERROR)),
            Cell::from(t.uploader.clone().unwrap_or_default()).style(Style::default().fg(ACCENT)),
        ])
    });

    let widths = [
        Constraint::Length(1),
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(SEL_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(Span::styled("▶ ", Style::default().fg(PRIMARY)))
        .column_spacing(1);

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_downloads(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BROWN))
        .title(Span::styled(
            format!(
                " Downloads · {} active → {} ",
                app.active_downloads(),
                app.output_dir.display()
            ),
            Style::default().fg(PRIMARY).bold(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); inner.height as usize])
        .split(inner);

    for (dl, row) in app
        .downloads
        .values()
        .take(inner.height as usize)
        .zip(rows.iter())
    {
        let ratio = if dl.total > 0 {
            (dl.downloaded as f64 / dl.total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let name = truncate(&dl.name, 34);
        let label = if let Some(err) = &dl.error {
            format!("✗ {name} — {err}")
        } else if dl.done {
            format!("✓ {name} — complete")
        } else {
            format!(
                "▼ {name} — {} / {} @ {}",
                human_bytes(dl.downloaded),
                human_bytes(dl.total),
                dl.speed.human_throughput_bytes()
            )
        };
        let color = if dl.error.is_some() {
            ERROR
        } else if dl.done {
            HIGHLIGHT
        } else {
            PRIMARY
        };
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(SEL_BG))
            .ratio(if dl.error.is_some() { 1.0 } else { ratio })
            .label(Span::styled(
                label,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(gauge, *row);
    }
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let hint = |k: &'static str, d: &'static str| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                format!(" {k} "),
                Style::default().fg(Color::Black).bg(PRIMARY).bold(),
            ),
            Span::styled(format!(" {d}  "), Style::default().fg(MUTED)),
        ]
    };

    let mut spans: Vec<Span> = Vec::new();
    if app.input_mode == InputMode::Editing {
        spans.extend(hint("Enter", "search"));
        spans.extend(hint("Esc", "cancel"));
        spans.extend(hint("⌫", "delete"));
    } else {
        spans.extend(hint("↑↓/jk", "move"));
        spans.extend(hint("←→", "category"));
        spans.extend(hint("/", "search"));
        spans.extend(hint("d", "download"));
        spans.extend(hint("c", "copy magnet"));
        spans.extend(hint("Enter", "details"));
        spans.extend(hint("?", "help"));
        spans.extend(hint("q", "quit"));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    let status_line = Line::from(vec![Span::styled(
        format!("{}  ", app.status),
        Style::default().fg(ACCENT),
    )])
    .alignment(Alignment::Right);
    f.render_widget(Paragraph::new(status_line), area);
}

fn render_detail(f: &mut Frame, app: &App) {
    let Some(t) = app.selected() else {
        return;
    };
    let area = centered_rect(72, 74, f.area());
    f.render_widget(Clear, area);

    let (icon, label) = t.category.label();
    let magnet = t.magnet();

    let field = |k: &str, v: String, color: Color| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {k:<11}"), Style::default().fg(ACCENT).bold()),
            Span::styled(v, Style::default().fg(color)),
        ])
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!("  {}", t.name),
            Style::default().fg(SKY).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        field("Category", format!("{icon} {label}"), TEXT),
        field("Size", human_bytes(t.size), SECONDARY),
        field("Seeders", t.seeders.to_string(), HIGHLIGHT),
        field("Leechers", t.leechers.to_string(), ERROR),
        field("Uploaded", human_date(t.added), MUTED),
        field("Uploader", or_dash(t.uploader.clone()), ACCENT),
        field("Files", or_dash(t.num_files.map(|n| n.to_string())), MUTED),
        field(
            "Downloads",
            or_dash(t.download_count.map(|n| n.to_string())),
            MUTED,
        ),
        field("Info hash", t.info_hash.clone(), MUTED),
        Line::from(""),
        Line::from(Span::styled("  Magnet", Style::default().fg(ACCENT).bold())),
        Line::from(Span::styled(
            format!(
                "  {}",
                truncate(&magnet, area.width.saturating_sub(4) as usize)
            ),
            Style::default().fg(LINK),
        )),
    ];

    if let Some(descr) = t.descr.as_ref().filter(|d| !d.trim().is_empty()) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Description",
            Style::default().fg(ACCENT).bold(),
        )));
        for l in descr.lines().take(6) {
            lines.push(Line::from(Span::styled(
                format!("  {l}"),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  d ", Style::default().fg(Color::Black).bg(PRIMARY).bold()),
        Span::styled(" download   ", Style::default().fg(MUTED)),
        Span::styled(" c ", Style::default().fg(Color::Black).bg(LINK).bold()),
        Span::styled(" copy magnet   ", Style::default().fg(MUTED)),
        Span::styled(" Esc ", Style::default().fg(Color::Black).bg(SKY).bold()),
        Span::styled(" close", Style::default().fg(MUTED)),
    ]));

    let popup = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(BROWN))
                .title(Span::styled(
                    " Torrent details ",
                    Style::default().fg(SKY).bold(),
                )),
        );
    f.render_widget(popup, area);
}

fn render_help(f: &mut Frame) {
    let area = centered_rect(56, 62, f.area());
    f.render_widget(Clear, area);

    let key = |k: &'static str, d: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("   {k:<12}"), Style::default().fg(SKY).bold()),
            Span::styled(d, Style::default().fg(TEXT)),
        ])
    };

    let lines = vec![
        Line::from(""),
        key("/  or  i", "focus the search box"),
        key("Enter", "run search / open details"),
        key("Esc", "leave search / close popup"),
        key("↑ ↓  j k", "move selection"),
        key("PgUp PgDn", "jump by a page"),
        key("g  G", "jump to top / bottom"),
        key("← →", "previous / next category"),
        key("d", "download the selected torrent"),
        key("c  or  y", "copy magnet link to clipboard"),
        key("r", "refresh current view"),
        key("?", "toggle this help"),
        key("q", "quit"),
        Line::from(""),
        Line::from(Span::styled(
            "   Downloads land in the shown output folder.",
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        )),
    ];

    let popup = Paragraph::new(Text::from(lines)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(BROWN))
            .title(Span::styled(
                " ☠ Keybindings ",
                Style::default().fg(SKY).bold(),
            )),
    );
    f.render_widget(popup, area);
}

// --- helpers ---------------------------------------------------------------

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn human_bytes(b: u64) -> String {
    b.human_count_bytes().to_string()
}

fn human_date(ts: Option<i64>) -> String {
    ts.and_then(|t| DateTime::from_timestamp(t, 0))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into())
}

fn or_dash(v: Option<String>) -> String {
    v.unwrap_or_else(|| "—".into())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_formats_sizes() {
        // human-repr uses SI units (1 kB = 1000 B).
        assert_eq!(human_bytes(1024), "1kB");
        assert_eq!(human_bytes(0), "0B");
        assert_eq!(human_bytes(2500000), "2.5MB");
    }

    #[test]
    fn human_date_formats_unix_timestamp() {
        assert_eq!(human_date(Some(1613347200)), "2021-02-15");
        // A missing timestamp yields an em dash placeholder.
        assert_eq!(human_date(None), "—");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
        // Counts characters, not bytes: multibyte input must not panic.
        assert_eq!(truncate("héllö", 10), "héllö");
    }
}
