pub mod albums;
pub mod chips;
pub mod header;
pub mod help;
pub mod info;
pub mod library;
pub mod now_playing;
pub mod playlists;
pub mod progress;
pub mod queue;
pub mod radio;
pub mod spectrum;
pub mod status;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, View};

/// Top-level render: persistent chrome (header → tabs → body → spectrum →
/// progress → status). The spectrum only appears when something's playing
/// and there's enough vertical real estate.
pub fn render(f: &mut Frame, app: &mut App) {
    app.hit_regions.clear();
    let area = f.area();
    let h = area.height;

    let header_h: u16 = if h < 22 { 3 } else { 5 };

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),    // header (3 boxes)
            Constraint::Length(1),           // tabs
            Constraint::Min(0),              // body
            Constraint::Length(1),           // progress bar (single row)
            Constraint::Length(1),           // hints + mpd info
        ])
        .split(area);

    header::render(f, app, v[0]);
    render_tabs(f, app, v[1]);
    if app.view != View::Radio { app.hit_regions.push((v[2], crate::input::Hit::Body)); }
    render_body(f, app, v[2]);
    progress::render(f, app, v[3]);
    status::render(f, app, v[4]);
}

fn render_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let order = [
        (View::Queue, "Queue"),
        (View::Albums, "Albums"),
        (View::Library, "Library"),
        (View::Playlists, "Playlists"),
        (View::NowPlaying, "Playing"),
        (View::Radio, "Radio"),
        (View::Info, "Info"),
    ];

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw(" "));
    let mut x = area.x + 1;
    for (v, label) in order {
        let width = label.len() as u16 + 2;
        app.hit_regions.push((Rect::new(x, area.y, width.min(area.right().saturating_sub(x)), 1), crate::input::Hit::Tab(v)));
        x += width + 1;
        let active = v == app.view;
        let chip = format!(" {label} ");
        let style = if active {
            Style::default()
                .fg(app.theme.bg_chip)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_muted)
        };
        spans.push(Span::styled(chip, style));
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    // Right-aligned lyrics toggle — only shown on Now Playing, since that's
    // the only view that renders the lyrics panel.
    if app.view == View::NowPlaying {
        let lyrics_chip = " L Lyrics ";
        let lyrics_style = if app.show_lyrics {
            Style::default()
                .fg(app.theme.bg_chip)
                .bg(app.theme.accent_alt)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_muted)
        };
        let lyrics_w = lyrics_chip.chars().count() as u16 + 1;
        if area.width > lyrics_w {
            let r = Rect {
                x: area.x + area.width - lyrics_w,
                y: area.y,
                width: lyrics_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Span::styled(lyrics_chip, lyrics_style)),
                r,
            );
        }
    }
}

fn render_body(f: &mut Frame, app: &mut App, area: Rect) {
    match app.view {
        View::Library => library::render(f, app, area),
        View::Albums => albums::render(f, app, area),
        View::Queue => queue::render(f, app, area),
        View::NowPlaying => now_playing::render(f, app, area),
        View::Radio => radio::render(f, app, area),
        View::Playlists => playlists::render(f, app, area),
        View::Info => info::render(f, app, area),
        View::Help => help::render(f, app, area),
    }
}

/// Hit rows are recorded after rendering, using the actual scroll offset.
pub fn hit_rows(app: &mut App, area: Rect, offset: usize, len: usize, make: fn(usize) -> crate::input::Hit) {
    for row in 0..area.height {
        let index = offset + row as usize;
        if index < len { app.hit_regions.push((Rect::new(area.x, area.y+row, area.width, 1), make(index))); }
    }
}
pub fn keys(items: &[(&str, &str)], app: &App) -> Line<'static> {
    let mut spans = vec![];
    for (i, (key, action)) in items.iter().enumerate() {
        if i > 0 { spans.push(Span::raw(" · ")); }
        spans.push(Span::styled(format!("[{key}]"), Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw(format!(" {action}")));
    }
    Line::from(spans)
}
